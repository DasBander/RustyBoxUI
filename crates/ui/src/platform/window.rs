//! Win32 window ownership, message pumping and frame/caption hit testing.
use super::{FRAME_TIMER, err, get};
use rustybox_core::Config;
use serde_json::json;
use std::{ffi::c_void, rc::Rc, sync::Mutex};
use windows::{
    Win32::{
        Foundation::*,
        System::{Com::*, LibraryLoader::*},
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
    core::{HSTRING, w},
};

static WINDOW_CLASS_USERS: Mutex<usize> = Mutex::new(0);
const CLASS: windows::core::PCWSTR = w!("RustyBoxUI.Window.v1");

struct WindowClass(HINSTANCE);
impl WindowClass {
    fn acquire() -> Result<Self, String> {
        let mut users = WINDOW_CLASS_USERS.lock().unwrap_or_else(|e| e.into_inner());
        let mut module = HMODULE::default();
        unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                windows::core::PCWSTR(window_proc as *const () as _),
                &mut module,
            )
            .map_err(err)?;
            let instance = HINSTANCE(module.0);
            if *users == 0 {
                let class = WNDCLASSW {
                    style: CS_DBLCLKS,
                    lpfnWndProc: Some(window_proc),
                    hInstance: instance,
                    lpszClassName: CLASS,
                    hCursor: LoadCursorW(None, IDC_ARROW).map_err(err)?,
                    ..Default::default()
                };
                if RegisterClassW(&class) == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
            }
            *users += 1;
            Ok(Self(instance))
        }
    }
}
impl Drop for WindowClass {
    fn drop(&mut self) {
        let mut users = WINDOW_CLASS_USERS.lock().unwrap_or_else(|e| e.into_inner());
        *users -= 1;
        if *users == 0 {
            unsafe {
                let _ = UnregisterClassW(CLASS, Some(self.0));
            }
        }
    }
}
pub(super) struct WindowOwner {
    pub(super) hwnd: HWND,
    _class: WindowClass,
}
impl Drop for WindowOwner {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(Some(self.hwnd)).as_bool() {
                let _ = DestroyWindow(self.hwnd);
            }
            CoUninitialize();
        }
    }
}
pub(super) fn create(config: Config, title: &str) -> Result<Rc<WindowOwner>, String> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| format!("Requires an STA UI thread: {e}"))?;
        let result = (|| {
            let class = WindowClass::acquire()?;
            let child = config.parent_window != 0;
            let style = if child {
                WS_CHILD | WS_CLIPSIBLINGS
            } else {
                WS_OVERLAPPEDWINDOW & !WS_CAPTION
            };
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                CLASS,
                &HSTRING::from(title),
                style,
                if child { 0 } else { CW_USEDEFAULT },
                if child { 0 } else { CW_USEDEFAULT },
                config.width as i32,
                config.height as i32,
                child.then_some(HWND(config.parent_window as *mut c_void)),
                None,
                Some(class.0),
                None,
            )
            .map_err(err)?;
            Ok(Rc::new(WindowOwner {
                hwnd,
                _class: class,
            }))
        })();
        if result.is_err() {
            CoUninitialize();
        }
        result
    }
}

pub fn pump() -> bool {
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
pub fn wait(milliseconds: u32) {
    unsafe {
        MsgWaitForMultipleObjectsEx(None, milliseconds, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let result = std::panic::catch_unwind(|| unsafe {
        let id = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as u64;
        if let Some(session) = get(id)
            && let Some(comp) = &session.composition
            && let Some(result) = comp.message(hwnd, msg, wparam, lparam)
        {
            return result;
        }
        match msg {
            WM_NCCALCSIZE => LRESULT(0),
            WM_NCHITTEST => {
                let point = POINT {
                    x: lparam.0 as i16 as i32,
                    y: (lparam.0 >> 16) as i16 as i32,
                };
                if (GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_CHILD.0) == 0
                    && !IsZoomed(hwnd).as_bool()
                    && let Some(edge) = resize_edge(hwnd, point)
                {
                    return LRESULT(edge as isize);
                }
                if let Some(session) = get(id)
                    && let Some(comp) = &session.composition
                    && comp.is_caption(hwnd, point)
                {
                    return LRESULT(HTCAPTION as isize);
                }
                LRESULT(HTCLIENT as isize)
            }
            WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK | WM_NCRBUTTONUP
                if wparam.0 == HTCAPTION as usize && GetAncestor(hwnd, GA_ROOT) != hwnd =>
            {
                // An embedded view's caption belongs to the host's top-level window.
                let _ = PostMessageW(Some(GetAncestor(hwnd, GA_ROOT)), msg, wparam, lparam);
                LRESULT(0)
            }
            WM_MOVE => {
                if let Some(session) = get(id)
                    && let Some(controller) = session.controller.borrow().as_ref()
                {
                    let _ = controller.NotifyParentWindowPositionChanged();
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == FRAME_TIMER => {
                // The OS move/size loop dispatches window messages while the host's poll is suspended.
                if let Some(session) = get(id)
                    && !IsIconic(GetAncestor(hwnd, GA_ROOT)).as_bool()
                {
                    session.present(false);
                }
                LRESULT(0)
            }
            WM_SIZE => {
                if let Some(session) = get(id) {
                    session.fit();
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Some(session) = get(id) {
                    session.shutdown();
                    session.emit(json!({"event":"closed"}));
                }
                LRESULT(0)
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lparam.0 as *mut MINMAXINFO);
                info.ptMinTrackSize = POINT { x: 960, y: 640 };
                use windows::Win32::Graphics::Gdi::{
                    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
                };
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let mut bounds = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                if GetMonitorInfoW(monitor, &mut bounds).as_bool() {
                    info.ptMaxPosition = POINT {
                        x: bounds.rcWork.left - bounds.rcMonitor.left,
                        y: bounds.rcWork.top - bounds.rcMonitor.top,
                    };
                    info.ptMaxSize = POINT {
                        x: bounds.rcWork.right - bounds.rcWork.left,
                        y: bounds.rcWork.bottom - bounds.rcWork.top,
                    };
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let rect = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    });
    result.unwrap_or(LRESULT(0))
}

fn resize_edge(hwnd: HWND, point: POINT) -> Option<u32> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
    let border = (6 * unsafe { GetDpiForWindow(hwnd) } / 96).max(4) as i32;
    let left = point.x < rect.left + border;
    let right = point.x >= rect.right - border;
    let top = point.y < rect.top + border;
    let bottom = point.y >= rect.bottom - border;
    Some(match (left, right, top, bottom) {
        (true, _, true, _) => HTTOPLEFT,
        (_, true, true, _) => HTTOPRIGHT,
        (true, _, _, true) => HTBOTTOMLEFT,
        (_, true, _, true) => HTBOTTOMRIGHT,
        (true, _, _, _) => HTLEFT,
        (_, true, _, _) => HTRIGHT,
        (_, _, true, _) => HTTOP,
        (_, _, _, true) => HTBOTTOM,
        _ => return None,
    })
}
