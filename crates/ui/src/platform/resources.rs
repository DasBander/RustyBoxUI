//! Serve encrypted web assets as same-origin, in-memory WebView2 responses.
use percent_encoding::percent_decode_str;
use rustybox_assets::Package;
use rustybox_core::AssetOrigin;
use std::rc::Rc;
use webview2_com::{
    CoTaskMemPWSTR, Microsoft::Web::WebView2::Win32::*, WebResourceRequestedEventHandler,
};
use windows::{
    Win32::UI::Shell::SHCreateMemStream,
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
fn byte_range(header: &str, len: usize) -> Option<std::ops::Range<usize>> {
    let (start, end) = header.strip_prefix("bytes=")?.split_once('-')?;
    if len == 0 {
        return None;
    }
    if start.is_empty() {
        let n = end.parse::<usize>().ok()?.min(len);
        return (n > 0).then_some(len - n..len);
    }
    let start = start.parse::<usize>().ok()?;
    let end = if end.is_empty() {
        len
    } else {
        end.parse::<usize>().ok()?.saturating_add(1).min(len)
    };
    (start < end).then_some(start..end)
}
pub fn mount(
    web: &ICoreWebView2,
    origin: AssetOrigin,
    package: Rc<Package>,
) -> windows::core::Result<()> {
    unsafe {
        let env = web.cast::<ICoreWebView2_2>()?.Environment()?;
        web.AddWebResourceRequestedFilter(
            &HSTRING::from(format!("{}/*", origin.base())),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        )?;
        let prefix = format!("{}/", origin.base());
        let mut token = 0;
        web.add_WebResourceRequested(&WebResourceRequestedEventHandler::create(Box::new(move |_,args| {
            let Some(args)=args else{return Ok(())};
            let req=args.Request()?;
            let mut uri=PWSTR::null();req.Uri(&mut uri)?;
            let uri=CoTaskMemPWSTR::from(uri).to_string();
            let raw=uri.strip_prefix(&prefix).unwrap_or("").split(['?','#']).next().unwrap_or("");
            let name=percent_decode_str(raw).decode_utf8().ok();
            let name=name.as_deref().unwrap_or("");
            let mut method=PWSTR::null();req.Method(&mut method)?;
            let method=CoTaskMemPWSTR::from(method).to_string();
            let (mut status, mut reason, bytes)=if !matches!(method.as_str(),"GET"|"HEAD") {(405,"Method Not Allowed",vec![])}
                else if !rustybox_assets::valid_name(name) || name.starts_with("_native/") {(404,"Not Found",vec![])}
                else {match package.read(name) {Ok(Some(data))=>(200,"OK",data), Ok(None)=>(404,"Not Found",vec![]),Err(_)=>(500,"Invalid Asset",vec![])}};
            let mut range=0..bytes.len();
            let mut extra=String::new();
            let mut requested=PWSTR::null();
            if status==200 && req.Headers()?.GetHeader(w!("Range"),&mut requested).is_ok() {
                let header=CoTaskMemPWSTR::from(requested).to_string();
                if !header.is_empty() {
                    if let Some(part)=byte_range(&header,bytes.len()){
                        extra=format!("Content-Range: bytes {}-{}/{}\r\n",part.start,part.end-1,bytes.len());
                        range=part;status=206;reason="Partial Content";
                    }else{extra=format!("Content-Range: bytes */{}\r\n",bytes.len());range=0..0;status=416;reason="Range Not Satisfiable";}
                }
            }
            let headers=format!("Content-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nAccept-Ranges: bytes\r\n{extra}",mime(name),range.len());
            let body=if method=="HEAD" {&[][..]} else {&bytes[range]};
            let stream=SHCreateMemStream(Some(body));
            let response=env.CreateWebResourceResponse(stream.as_ref(),status,&HSTRING::from(reason),&HSTRING::from(headers))?;
            args.SetResponse(&response)
        })),&mut token)
    }
}
