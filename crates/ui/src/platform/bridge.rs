//! Dispatch trusted page messages to built-ins, storage workers or the host queue.
use super::{Session, err};
use rustybox_core::{parse_request, response};
use serde_json::{Value, json};
use webview2_com::CoTaskMemPWSTR;
use windows::{
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
    core::{HSTRING, PWSTR},
};

impl Session {
    pub fn emit(&self, event: Value) {
        let mut queue = self.events.borrow_mut();
        if queue.len() < 256 {
            queue.push_back(event.to_string().into_bytes());
        }
    }
    pub fn post(&self, value: Value) -> Result<(), String> {
        let web = self.webview.borrow().as_ref().ok_or("View closed")?.clone();
        unsafe {
            let mut source = PWSTR::null();
            web.Source(&mut source).map_err(err)?;
            if !self
                .origin
                .trusts(&CoTaskMemPWSTR::from(source).to_string())
            {
                return Err("View has no trusted document".into());
            }
            web.PostWebMessageAsJson(&HSTRING::from(value.to_string()))
                .map_err(err)
        }
    }
    pub(super) fn dispatch(&self, raw: &str) {
        let request = match parse_request(raw) {
            Ok(r) => r,
            Err(e) => {
                self.emit(json!({"event":"protocol_error","message":e}));
                return;
            }
        };
        let id = request.id;
        let session = request.session.clone();
        // A host invocation stays queued until the embedding app drains rb_next_event.
        if request.method.starts_with("host.") {
            if self.events.borrow().len() >= 256 {
                let _ = self.post(response(id, &session, Err("Host event queue full".into())));
                return;
            }
            self.emit(json!({
                "event": "invoke",
                "request": {
                    "v": 1, "id": id, "session": session,
                    "method": request.method, "params": request.params,
                }
            }));
            return;
        }
        if matches!(request.method.as_str(), "storage.get" | "storage.set") {
            if let Err(e) = self.storage.submit(request) {
                let _ = self.post(response(id, &session, Err(e)));
            }
            return;
        }
        let result = self.command(&request.method, request.params);
        let _ = self.post(response(id, &session, result));
    }
    fn command(&self, method: &str, params: Value) -> Result<Value, String> {
        let top = unsafe { GetAncestor(self.hwnd, GA_ROOT) };
        match method {
            "runtime.info" => Ok(json!({
                "name": "RustyBoxUI",
                "version": env!("CARGO_PKG_VERSION"),
                "abi": rustybox_core::ABI_VERSION,
                "platform": "windows-x64",
                "engine": "WebView2",
                "browserVersion": self.browser_version,
                "graphicsAvailable": self.graphics_path.is_file(),
                "uptimeMs": self.started.elapsed().as_millis(),
                "debug": self.debug,
                "composition": self.composition.is_some(),
            })),
            "diagnostics.echo" => Ok(params),
            "window.minimize" => {
                unsafe {
                    let _ = ShowWindow(top, SW_MINIMIZE);
                }
                Ok(Value::Null)
            }
            "window.maximize" => {
                unsafe {
                    let _ = ShowWindow(
                        top,
                        if IsZoomed(top).as_bool() {
                            SW_RESTORE
                        } else {
                            SW_MAXIMIZE
                        },
                    );
                }
                Ok(Value::Null)
            }
            "window.close" => {
                unsafe {
                    PostMessageW(Some(top), WM_CLOSE, WPARAM(0), LPARAM(0)).map_err(err)?;
                }
                Ok(Value::Null)
            }
            "window.drag" => {
                unsafe {
                    use windows::Win32::UI::Input::KeyboardAndMouse::{
                        GetAsyncKeyState, GetCapture, ReleaseCapture, VK_LBUTTON,
                    };
                    // The DOM request is asynchronous. Ignore a press already released.
                    if GetAsyncKeyState(VK_LBUTTON.0.into()) >= 0 {
                        return Ok(Value::Null);
                    }
                    let mut point = POINT::default();
                    GetCursorPos(&mut point).map_err(err)?;
                    if !GetCapture().is_invalid() {
                        let _ = ReleaseCapture();
                    }
                    let position = u32::from(point.x as u16) | (u32::from(point.y as u16) << 16);
                    PostMessageW(
                        Some(top),
                        WM_NCLBUTTONDOWN,
                        WPARAM(HTCAPTION as usize),
                        LPARAM(position as isize),
                    )
                    .map_err(err)?;
                }
                Ok(Value::Null)
            }
            "graphics.detach" => {
                self.detach_surface(super::viewport::slot_id(&params)?);
                if let Some(c) = &self.composition {
                    c.commit()?;
                }
                Ok(json!({"active":false}))
            }
            "graphics.info" => Ok(self.graphics_info(super::viewport::slot_id(&params)?)),
            "graphics.attach" => self.attach(params),
            "graphics.view" => {
                let zoom = params["zoom"].as_f64().ok_or("Missing zoom")? as f32;
                let channel = match params["channel"].as_str() {
                    Some("rgb") => 0,
                    Some("red") => 1,
                    Some("green") => 2,
                    Some("blue") => 3,
                    Some("alpha") => 4,
                    _ => return Err("Invalid channel".into()),
                };
                let motion = params["motion"].as_bool().ok_or("Missing motion setting")?;
                self.with_graphics(super::viewport::slot_id(&params)?, |g| {
                    g.options(zoom, channel, motion)
                })?;
                Ok(params)
            }
            _ => Err(format!("Unknown command: {method}")),
        }
    }
}
