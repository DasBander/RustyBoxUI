//! Thread-affine view registry and shutdown for the Windows runtime.
mod bridge;
mod fullscreen;
mod resource_stream;
mod resources;
mod shell;
mod startup;
mod viewport;
mod webview;
mod window;
pub use startup::{begin, cancel, collect, finish};
pub use window::{pump, wait};

use crate::{composition::Composition, storage::Storage};
use rustybox_core::AssetOrigin;
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    path::PathBuf,
    rc::Rc,
    time::Instant,
};
use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2Controller};
use window::WindowOwner;
use windows::Win32::Foundation::HWND;

thread_local! {
    // HWNDs and WebView2 controllers belong to the creating apartment.
    pub static SESSIONS: RefCell<HashMap<u64, Rc<Session>>> = RefCell::new(HashMap::new());
}
const FRAME_TIMER: usize = 1;
const FRAME_INTERVAL_MS: u32 = 16;

pub struct Session {
    pub hwnd: HWND,
    origin: AssetOrigin,
    pub closed: Cell<bool>,
    fullscreen: Cell<Option<fullscreen::Windowed>>,
    host_fullscreen: Cell<bool>,
    page_fullscreen: Cell<bool>,
    shell: RefCell<shell::Shell>,
    controller: RefCell<Option<ICoreWebView2Controller>>,
    webview: RefCell<Option<ICoreWebView2>>,
    pub events: RefCell<VecDeque<Vec<u8>>>,
    storage: Storage,
    surfaces: RefCell<HashMap<u32, Rc<viewport::Surface>>>,
    composition: Option<Composition>,
    started: Instant,
    last_frame: Cell<Instant>,
    graphics_path: PathBuf,
    browser_version: String,
    debug: bool,
    _owner: Rc<WindowOwner>,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn get(id: u64) -> Option<Rc<Session>> {
    SESSIONS.with(|s| s.borrow().get(&id).cloned())
}

impl Session {
    pub fn shutdown(&self) {
        self.closed.set(true);
        self.shell.borrow_mut().remove(self.hwnd);
        let ids: Vec<_> = self.surfaces.borrow().keys().copied().collect();
        for id in ids {
            self.detach_surface(id);
        }
        self.webview.borrow_mut().take();
        if let Some(controller) = self.controller.borrow_mut().take() {
            unsafe {
                let _ = controller.Close();
            }
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.shutdown();
    }
}
pub fn destroy(id: u64) -> bool {
    let session = SESSIONS.with(|s| s.borrow_mut().remove(&id));
    if let Some(session) = session {
        session.shutdown();
        drop(session);
        true
    } else {
        false
    }
}
