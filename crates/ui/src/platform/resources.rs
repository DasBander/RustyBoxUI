//! Serve authenticated, seekable web assets at the application origin.
use super::resource_stream;
use percent_encoding::percent_decode_str;
use rustybox_assets::Package;
use rustybox_core::AssetOrigin;
use std::{
    io::{Read, Seek},
    ops::Range,
    rc::Rc,
};
use webview2_com::{
    CoTaskMemPWSTR, Microsoft::Web::WebView2::Win32::*, WebResourceRequestedEventHandler,
};
use windows::{
    Win32::System::Com::IStream,
    core::{HSTRING, Interface, PWSTR, w},
};

fn mime(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
// A decimal HTTP endpoint can exceed u64. Saturation still gives the right result when clamped to EOF.
fn range_number(text: &str) -> Option<u64> {
    if text.is_empty() {
        return None;
    }
    text.bytes().try_fold(0u64, |n, digit| {
        digit
            .is_ascii_digit()
            .then(|| n.saturating_mul(10).saturating_add(u64::from(digit - b'0')))
    })
}

fn byte_range(header: &str, len: u64) -> Option<Range<u64>> {
    let (unit, span) = header.trim().split_once('=')?;
    let (start, end) = span.split_once('-')?;
    if len == 0 || !unit.eq_ignore_ascii_case("bytes") {
        return None;
    }
    if start.is_empty() {
        let n = range_number(end)?.min(len);
        return (n > 0).then_some(len - n..len);
    }
    let start = range_number(start)?;
    let end = if end.is_empty() {
        len
    } else {
        range_number(end)?.saturating_add(1).min(len)
    };
    (start < end).then_some(start..end)
}
struct Response {
    status: i32,
    reason: &'static str,
    length: u64,
    content_range: Option<String>,
    body: Option<IStream>,
}

impl Response {
    fn empty(status: i32, reason: &'static str) -> Self {
        Self {
            status,
            reason,
            length: 0,
            content_range: None,
            body: None,
        }
    }
}

fn respond(package: &Package, name: &str, method: &str, requested: &str) -> Response {
    if !matches!(method, "GET" | "HEAD") {
        return Response::empty(405, "Method Not Allowed");
    }
    if !rustybox_assets::valid_name(name) || name.starts_with("_native/") {
        return Response::empty(404, "Not Found");
    }
    let Some(length) = package.asset_len(name) else {
        return Response::empty(404, "Not Found");
    };
    let (range, mut response) = if requested.is_empty() {
        (0..length, Response::empty(200, "OK"))
    } else if let Some(range) = byte_range(requested, length) {
        let mut response = Response::empty(206, "Partial Content");
        response.content_range = Some(format!(
            "bytes {}-{}/{}",
            range.start,
            range.end - 1,
            length
        ));
        (range, response)
    } else {
        let mut response = Response::empty(416, "Range Not Satisfiable");
        response.content_range = Some(format!("bytes */{length}"));
        return response;
    };
    response.length = range.end - range.start;
    if method == "GET" {
        match open_body(package, name, range) {
            Ok(stream) => response.body = stream,
            Err(_) => return Response::empty(500, "Invalid Asset"),
        }
    }
    response
}

fn open_body(package: &Package, name: &str, range: Range<u64>) -> Result<Option<IStream>, String> {
    let reader = package.reader(name)?.ok_or("Missing asset")?;
    let mut reader = reader.range(range).map_err(|e| e.to_string())?;
    if reader.is_empty() {
        return Ok(None);
    }
    // Check the first requested block before sending HTTP headers. Further blocks authenticate on read.
    reader.read_exact(&mut [0]).map_err(|e| e.to_string())?;
    reader.rewind().map_err(|e| e.to_string())?;
    resource_stream::open(reader)
        .map(Some)
        .map_err(|e| e.to_string())
}

pub fn mount(
    web: &ICoreWebView2,
    origin: AssetOrigin,
    package: Rc<Package>,
) -> windows::core::Result<()> {
    unsafe {
        let environment = web.cast::<ICoreWebView2_2>()?.Environment()?;
        web.AddWebResourceRequestedFilter(
            &HSTRING::from(format!("{}/*", origin.base())),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        )?;
        let prefix = format!("{}/", origin.base());
        let handler = WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let request = args.Request()?;
            let mut uri = PWSTR::null();
            request.Uri(&mut uri)?;
            let uri = CoTaskMemPWSTR::from(uri).to_string();
            let path = uri
                .strip_prefix(&prefix)
                .unwrap_or("")
                .split(['?', '#'])
                .next()
                .unwrap_or("");
            let name = percent_decode_str(path).decode_utf8().unwrap_or_default();
            let mut method = PWSTR::null();
            request.Method(&mut method)?;
            let method = CoTaskMemPWSTR::from(method).to_string();
            let mut value = PWSTR::null();
            let headers = request.Headers()?;
            let range = if headers.GetHeader(w!("Range"), &mut value).is_ok() {
                CoTaskMemPWSTR::from(value).to_string()
            } else {
                String::new()
            };
            let reply = respond(&package, &name, &method, &range);
            let mut headers = format!(
                "Content-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nAccept-Ranges: bytes\r\n",
                mime(&name),
                reply.length
            );
            if let Some(value) = reply.content_range {
                headers.push_str(&format!("Content-Range: {value}\r\n"));
            }
            let response = environment.CreateWebResourceResponse(
                reply.body.as_ref(),
                reply.status,
                &HSTRING::from(reply.reason),
                &HSTRING::from(headers),
            )?;
            args.SetResponse(&response)
        }));
        let mut token = 0;
        web.add_WebResourceRequested(&handler, &mut token)
    }
}
