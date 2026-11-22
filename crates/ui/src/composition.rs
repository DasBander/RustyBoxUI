use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{DirectComposition::*, Gdi::ScreenToClient},
        UI::{Accessibility::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::{IUnknown, Interface},
};

/// The GPU swap chain sits below the transparent web visual. Input belongs to the DOM.
pub struct Composition {
    pub surface: IDCompositionVisual,
    device: IDCompositionDesktopDevice,
    controller: ICoreWebView2CompositionController,
    regions: Option<ICoreWebView2CompositionController4>,
    _target: IDCompositionTarget,
}
impl Composition {
    pub fn new(
        hwnd: HWND,
        controller: ICoreWebView2CompositionController,
    ) -> windows::core::Result<Self> {
        unsafe {
            let device: IDCompositionDesktopDevice = DCompositionCreateDevice2(None::<&IUnknown>)?;
            let target = device.CreateTargetForHwnd(hwnd, true)?;
            let root = device.CreateVisual()?;
            let surface: IDCompositionVisual = device.CreateVisual()?.cast()?;
            let web = device.CreateVisual()?;
            root.AddVisual(&surface, false, None)?;
            root.AddVisual(&web, true, &surface)?;
            controller.SetRootVisualTarget(&web)?;
            target.SetRoot(&root)?;
            device.Commit()?;
            Ok(Self {
                surface,
                device,
                regions: controller.cast().ok(),
                controller,
                _target: target,
            })
        }
    }
    pub fn commit(&self) -> Result<(), String> {
        unsafe { self.device.Commit() }.map_err(|e| e.to_string())
    }
    pub fn is_caption(&self, hwnd: HWND, mut screen_point: POINT) -> bool {
        let Some(regions) = &self.regions else {
            return false;
        };
        let mut kind = COREWEBVIEW2_NON_CLIENT_REGION_KIND_NOWHERE;
        // Controller bounds fill the client area. WebView2 expects client pixels at the current DPI.
        unsafe {
            ScreenToClient(hwnd, &mut screen_point).as_bool()
                && regions
                    .GetNonClientRegionAtPoint(screen_point, &mut kind)
                    .is_ok()
                && kind == COREWEBVIEW2_NON_CLIENT_REGION_KIND_CAPTION
        }
    }
    pub fn message(&self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        const MOUSE_LEAVE: u32 = 0x02a3;
        unsafe {
            match msg {
                WM_GETOBJECT if lparam.0 as i32 == UiaRootObjectId => {
                    let extended: ICoreWebView2CompositionController2 =
                        self.controller.cast().ok()?;
                    let provider: IRawElementProviderSimple =
                        extended.AutomationProvider().ok()?.cast().ok()?;
                    return Some(UiaReturnRawElementProvider(hwnd, wparam, lparam, &provider));
                }
                WM_SETCURSOR if (lparam.0 as u16) as u32 == HTCLIENT => {
                    let mut cursor = HCURSOR::default();
                    self.controller.Cursor(&mut cursor).ok()?;
                    SetCursor(Some(cursor));
                    return Some(LRESULT(1));
                }
                WM_SETFOCUS => {
                    self.focus();
                    return Some(LRESULT(0));
                }
                WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK
                | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK | WM_MBUTTONDOWN
                | WM_MBUTTONUP | WM_MBUTTONDBLCLK | WM_XBUTTONDOWN | WM_XBUTTONUP
                | WM_XBUTTONDBLCLK | WM_MOUSEWHEEL | WM_MOUSEHWHEEL | MOUSE_LEAVE => (),
                _ => return None,
            }
            let mut point = POINT {
                x: lparam.0 as i16 as i32,
                y: (lparam.0 >> 16) as i16 as i32,
            };
            if matches!(msg, WM_MOUSEWHEEL | WM_MOUSEHWHEEL) {
                let _ = ScreenToClient(hwnd, &mut point);
            }
            if matches!(
                msg,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            ) {
                self.focus();
                SetCapture(hwnd);
            }
            if matches!(
                msg,
                WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP | WM_XBUTTONUP
            ) && wparam.0 & 0x73 == 0
                && GetCapture() == hwnd
            {
                let _ = ReleaseCapture();
            }
            if msg == WM_MOUSEMOVE {
                let mut track = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    ..Default::default()
                };
                let _ = TrackMouseEvent(&mut track);
            }
            let data = match msg {
                WM_MOUSEWHEEL | WM_MOUSEHWHEEL => (wparam.0 >> 16) as i16 as i32 as u32,
                WM_XBUTTONDOWN | WM_XBUTTONUP | WM_XBUTTONDBLCLK => (wparam.0 >> 16) as u16 as u32,
                _ => 0,
            };
            self.controller
                .SendMouseInput(
                    COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32),
                    COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS((wparam.0 & 0x7f) as i32),
                    data,
                    point,
                )
                .ok()?;
            Some(LRESULT(0))
        }
    }
    fn focus(&self) {
        if let Ok(controller) = self.controller.cast::<ICoreWebView2Controller>() {
            unsafe {
                let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            }
        }
    }
}
impl Drop for Composition {
    fn drop(&mut self) {
        unsafe {
            let _ = self.controller.SetRootVisualTarget(None::<&IUnknown>);
            let _ = self.device.Commit();
        }
    }
}
