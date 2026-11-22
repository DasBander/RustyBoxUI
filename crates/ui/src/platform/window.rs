//! Win32 window ownership, message pumping and frame/caption hit testing.
use super::{FRAME_TIMER, err, get};
use rustybox_core::Config;
use serde_json::json;
use std::{ffi::c_void, rc::Rc, sync::Mutex};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::{
            GetMonitorInfoW, HMONITOR, IntersectRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
            MonitorFromRect, MonitorFromWindow,
        },
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
                // Branding belongs to the host executable, while the window procedure lives in this DLL.
                let icon = GetModuleHandleW(None)
                    .and_then(|host| {
                        LoadIconW(Some(HINSTANCE(host.0)), windows::core::PCWSTR(1 as _))
                    })
                    .unwrap_or_default();
                let class = WNDCLASSW {
                    hIcon: icon,
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
        let active_session = get(id);
        if let Some(session) = &active_session
            && let Some(comp) = &session.composition
            && let Some(result) = comp.message(hwnd, msg, wparam, lparam)
        {
            return result;
        }
        let fullscreen = active_session
            .as_ref()
            .is_some_and(|session| session.fullscreen.get().is_some());
        match msg {
            WM_NCHITTEST if fullscreen => LRESULT(HTCLIENT as isize),
            WM_SYSCOMMAND
                if fullscreen
                    && matches!((wparam.0 & 0xfff0) as u32, SC_SIZE | SC_MOVE | SC_MAXIMIZE) =>
            {
                LRESULT(0)
            }
            WM_NCCALCSIZE => {
                if !fullscreen
                    && IsZoomed(hwnd).as_bool()
                    && (GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_CHILD.0) == 0
                {
                    // A maximized thick frame can extend beyond the monitor by its resize border.
                    // Keep the web client inside the work area, including on a monitor with a taskbar.
                    let client = if wparam.0 != 0 {
                        &mut (*(lparam.0 as *mut NCCALCSIZE_PARAMS)).rgrc[0]
                    } else {
                        &mut *(lparam.0 as *mut RECT)
                    };
                    let monitor = MonitorFromRect(client, MONITOR_DEFAULTTONEAREST);
                    let mut visible = RECT::default();
                    if let Some(bounds) = monitor_bounds(monitor)
                        && IntersectRect(&mut visible, client, &bounds.rcWork).as_bool()
                    {
                        *client = visible;
                    }
                }
                LRESULT(0)
            }
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
            windows::Win32::UI::WindowsAndMessaging::WM_DROPFILES => {
                use windows::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
                let drop = HDROP(wparam.0 as *mut _);
                let mut paths = Vec::new();
                for index in 0..DragQueryFileW(drop, u32::MAX, None).min(256) {
                    let length = DragQueryFileW(drop, index, None);
                    let mut path = vec![0; length as usize + 1];
                    let count = DragQueryFileW(drop, index, Some(&mut path));
                    paths.push(String::from_utf16_lossy(&path[..count as usize]));
                }
                DragFinish(drop);
                if let Some(session) = get(id) {
                    session.emit(json!({"event":"files_dropped","paths":paths}));
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
                info.ptMinTrackSize = if fullscreen {
                    POINT { x: 1, y: 1 }
                } else {
                    POINT { x: 960, y: 640 }
                };
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                if let Some(bounds) = monitor_bounds(monitor) {
                    let area = if fullscreen {
                        bounds.rcMonitor
                    } else {
                        bounds.rcWork
                    };
                    info.ptMaxPosition = POINT {
                        x: area.left - bounds.rcMonitor.left,
                        y: area.top - bounds.rcMonitor.top,
                    };
                    info.ptMaxSize = POINT {
                        x: area.right - area.left,
                        y: area.bottom - area.top,
                    };
                }
                LRESULT(0)
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED if fullscreen => {
                let monitor = if msg == WM_DPICHANGED {
                    MonitorFromRect(lparam.0 as *const RECT, MONITOR_DEFAULTTONEAREST)
                } else {
                    MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)
                };
                if let Some(bounds) = monitor_bounds(monitor) {
                    let _ = super::fullscreen::fit_monitor(hwnd, bounds.rcMonitor);
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

pub(super) fn monitor_bounds(monitor: HMONITOR) -> Option<MONITORINFO> {
    let mut bounds = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut bounds) }
        .as_bool()
        .then_some(bounds)
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
