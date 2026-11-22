//! Public Rust host SDK. All native UI functionality is loaded from the distributed DLL.
mod commands;
pub use commands::Commands;

use libloading::Library;
pub use rustybox_core::AssetOrigin;
use rustybox_core::{ABI_VERSION, CLOSED, Config, EMPTY, HIDDEN, MAX_MESSAGE, PENDING, SELF_TEST};
use serde_json::Value;
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D},
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
    core::{HSTRING, Interface},
};
pub type Result<T> = std::result::Result<T, String>;
type Begin = unsafe extern "C" fn(*const Config, u32, *mut u64) -> i32;
type Call = unsafe extern "C" fn(u64) -> i32;
type EventFn = unsafe extern "C" fn(u64, *mut u8, usize, *mut usize) -> i32;
type Post = unsafe extern "C" fn(u64, *const u8, usize) -> i32;
struct Api {
    begin: Begin,
    status: Call,
    cancel: Call,
    collect: unsafe extern "C" fn() -> i32,
    poll: Call,
    destroy: Call,
    reset: Call,
    wait: unsafe extern "C" fn(u64, u32) -> i32,
    event: EventFn,
    post: Post,
    hwnd: unsafe extern "C" fn(u64) -> usize,
    device: unsafe extern "C" fn(u64, *mut *mut c_void) -> i32,
    texture: unsafe extern "C" fn(u64, *mut c_void) -> i32,
    error: unsafe extern "C" fn(*mut u8, usize) -> usize,
    _lib: Library,
}
impl Api {
    fn load(path: &Path) -> Result<Rc<Self>> {
        unsafe {
            let lib =
                Library::new(path).map_err(|e| format!("Cannot load {}: {e}", path.display()))?;
            let version = lib
                .get::<unsafe extern "C" fn() -> u32>(b"rb_abi_version\0")
                .map_err(|e| e.to_string())?;
            if version() != ABI_VERSION {
                return Err("Incompatible RustyBoxUI ABI".into());
            }
            macro_rules! load {
                ($name:literal) => {
                    *lib.get(concat!($name, "\0").as_bytes())
                        .map_err(|e| e.to_string())?
                };
            }
            Ok(Rc::new(Self {
                begin: load!("rb_create_begin"),
                status: load!("rb_create_status"),
                cancel: load!("rb_create_cancel"),
                collect: load!("rb_collect_startups"),
                poll: load!("rb_poll"),
                destroy: load!("rb_destroy"),
                reset: load!("rb_graphics_reset"),
                wait: load!("rb_wait"),
                event: load!("rb_next_event"),
                post: load!("rb_post_json"),
                hwnd: load!("rb_native_handle"),
                device: load!("rb_graphics_device"),
                texture: load!("rb_graphics_texture"),
                error: load!("rb_last_error"),
                _lib: lib,
            }))
        }
    }
    fn check(&self, status: i32) -> Result<i32> {
        if status >= 0 {
            return Ok(status);
        }
        let mut bytes = [0u8; 2048];
        let len = unsafe { (self.error)(bytes.as_mut_ptr(), bytes.len()) }.min(bytes.len() - 1);
        Err(format!(
            "RustyBoxUI {status}: {}",
            String::from_utf8_lossy(&bytes[..len])
        ))
    }
}
/// Paths and display options used to open one native view on the current UI thread.
pub struct Window {
    pub title: String,
    pub assets: PathBuf,
    pub profile: PathBuf,
    pub dll: PathBuf,
    pub size: (u32, u32),
    pub composed: bool,
    pub self_test: bool,
    pub hidden: bool,
}
impl Window {
    pub fn new(title: &str, assets: PathBuf, profile: PathBuf) -> Result<Self> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        Ok(Self {
            title: title.into(),
            assets,
            profile,
            dll: exe.with_file_name("rustybox_ui.dll"),
            size: (1344, 862),
            composed: false,
            self_test: false,
            hidden: false,
        })
    }
    pub fn begin(self, timeout: Duration) -> Result<Startup> {
        self.begin_with_origin(timeout, AssetOrigin::LegacyLocal)
    }
    pub fn begin_with_origin(self, timeout: Duration, origin: AssetOrigin) -> Result<Startup> {
        let api = Api::load(&self.dll)?;
        let assets = self.assets.to_string_lossy();
        let profile = self.profile.to_string_lossy();
        // Public sizes are logical pixels; respect the active desktop's working area.
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        let width = (self.size.0.saturating_mul(dpi) / 96)
            .min(unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(320) as u32);
        let height = (self.size.1.saturating_mul(dpi) / 96)
            .min((unsafe { GetSystemMetrics(SM_CYSCREEN) } - 60).max(240) as u32);
        let config = Config {
            width,
            height,
            flags: if self.composed {
                rustybox_core::COMPOSED
            } else {
                0
            } | if self.self_test { SELF_TEST } else { 0 }
                | if self.hidden { HIDDEN } else { 0 }
                | match origin {
                    AssetOrigin::Reserved => rustybox_core::RESERVED_ORIGIN,
                    AssetOrigin::LegacyLocal => 0,
                },
            title: self.title.as_ptr(),
            title_len: self.title.len(),
            assets: assets.as_ptr(),
            assets_len: assets.len(),
            data_dir: profile.as_ptr(),
            data_dir_len: profile.len(),
            ..Default::default()
        };
        let mut id = 0;
        let milliseconds =
            u32::try_from(timeout.as_millis()).map_err(|_| "Startup deadline out of range")?;
        api.check(unsafe { (api.begin)(&config, milliseconds, &mut id) })?;
        Ok(Startup { api, id: Some(id) })
    }
    pub fn open(self) -> Result<View> {
        self.open_with_origin(AssetOrigin::LegacyLocal)
    }
    pub fn open_with_origin(self, origin: AssetOrigin) -> Result<View> {
        let mut startup = self.begin_with_origin(Duration::from_secs(30), origin)?;
        loop {
            if !pump_messages() {
                return Err("Window closed during startup".into());
            }
            if let Some(view) = startup.try_finish()? {
                return Ok(view);
            }
            unsafe {
                MsgWaitForMultipleObjectsEx(None, 8, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
        }
    }
}
pub fn enable_per_monitor_dpi() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}
/// Call on the creating thread after pumping messages following a cancelled or expired startup.
/// Returns true once callbacks from this DLL have drained and their native resources are released.
pub fn collect_startups(dll: &Path) -> Result<bool> {
    let api = Api::load(dll)?;
    Ok(api.check(unsafe { (api.collect)() })? != PENDING)
}
pub fn pump_messages() -> bool {
    unsafe {
        let mut msg = MSG::default();
        for _ in 0..256 {
            if !PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                break;
            }
            if msg.message == WM_QUIT {
                PostQuitMessage(msg.wParam.0 as i32);
                return false;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    true
}
/// A pending WebView2 startup. Poll it while dispatching messages on the creating STA.
pub struct Startup {
    api: Rc<Api>,
    id: Option<u64>,
}
impl Startup {
    /// Call between turns of the creating STA's message loop. Dropping an unfinished startup cancels it.
    pub fn try_finish(&mut self) -> Result<Option<View>> {
        let id = self.id.ok_or("Startup already consumed")?;
        let status = unsafe { (self.api.status)(id) };
        if status == PENDING {
            return Ok(None);
        }
        self.id = None;
        self.api.check(status)?;
        Ok(Some(View {
            api: self.api.clone(),
            id,
            buffer: Vec::new(),
        }))
    }
    pub fn cancel(mut self) -> Result<()> {
        let id = self.id.take().ok_or("Startup already consumed")?;
        self.api.check(unsafe { (self.api.cancel)(id) })?;
        Ok(())
    }
}
impl Drop for Startup {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            unsafe {
                (self.api.cancel)(id);
            }
        }
    }
}
/// Owns the native view and keeps its DLL loaded until the handle is destroyed.
pub struct View {
    api: Rc<Api>,
    id: u64,
    buffer: Vec<u8>,
}
impl View {
    pub fn hwnd(&self) -> HWND {
        HWND(unsafe { (self.api.hwnd)(self.id) } as *mut _)
    }
    pub fn device(&self) -> Result<ID3D11Device> {
        let mut raw = std::ptr::null_mut();
        self.api
            .check(unsafe { (self.api.device)(self.id, &mut raw) })?;
        Ok(unsafe { ID3D11Device::from_raw(raw) })
    }
    pub fn set_texture(&self, texture: Option<&ID3D11Texture2D>) -> Result<()> {
        self.api.check(unsafe {
            (self.api.texture)(
                self.id,
                texture.map_or(std::ptr::null_mut(), Interface::as_raw),
            )
        })?;
        Ok(())
    }
    pub fn reset_graphics(&self) -> Result<()> {
        self.api.check(unsafe { (self.api.reset)(self.id) })?;
        Ok(())
    }
    pub fn poll(&self) -> Result<bool> {
        Ok(self.api.check(unsafe { (self.api.poll)(self.id) })? != CLOSED)
    }
    pub fn wait(&self, milliseconds: u32) -> Result<()> {
        self.api
            .check(unsafe { (self.api.wait)(self.id, milliseconds) })?;
        Ok(())
    }
    pub fn next_event(&mut self) -> Result<Option<Value>> {
        let mut len = 0;
        let status = unsafe { (self.api.event)(self.id, std::ptr::null_mut(), 0, &mut len) };
        if status == EMPTY {
            return Ok(None);
        }
        if status != rustybox_core::BUFFER_TOO_SMALL {
            self.api.check(status)?;
        }
        if len > MAX_MESSAGE + 4096 {
            return Err("Oversize host event".into());
        }
        self.buffer.resize(len, 0);
        self.api
            .check(unsafe { (self.api.event)(self.id, self.buffer.as_mut_ptr(), len, &mut len) })?;
        Ok(Some(
            serde_json::from_slice(&self.buffer[..len]).map_err(|e| e.to_string())?,
        ))
    }
    pub fn post(&self, value: &Value) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        self.api
            .check(unsafe { (self.api.post)(self.id, bytes.as_ptr(), bytes.len()) })?;
        Ok(())
    }
}
impl Drop for View {
    fn drop(&mut self) {
        unsafe {
            (self.api.destroy)(self.id);
        }
    }
}

/// Show a GUI startup diagnostic when launched from Explorer.
pub fn show_error(message: &str) {
    unsafe {
        MessageBoxW(
            None,
            &HSTRING::from(message),
            &HSTRING::from("RustyBoxUI"),
            MB_ICONERROR | MB_OK,
        );
    }
}
