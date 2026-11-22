//! Match a standalone host window to an HTML fullscreen element on its current monitor.
use super::{Session, window::monitor_bounds};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow},
        UI::WindowsAndMessaging::*,
    },
    core::{Error, Result},
};

#[derive(Clone, Copy)]
pub(super) struct Windowed {
    style: u32,
    placement: WINDOWPLACEMENT,
    visible: bool,
}

pub(super) fn fit_monitor(hwnd: HWND, rect: RECT) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        )
    }
}

impl Session {
    // A fullscreen video may open inside a game that already requested a borderless window.
    // Releasing either request preserves the other until its owner also releases it.
    pub(super) fn request_fullscreen(
        &self,
        owner: &std::cell::Cell<bool>,
        enabled: bool,
    ) -> Result<()> {
        let previous = owner.replace(enabled);
        self.set_fullscreen(self.host_fullscreen.get() || self.page_fullscreen.get())
            .inspect_err(|_| owner.set(previous))
    }

    fn set_fullscreen(&self, active: bool) -> Result<()> {
        let hwnd = self.hwnd;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
            // A child view fills its own surface. Its embedding application owns the top-level HWND.
            if style & WS_CHILD.0 != 0 || active == self.fullscreen.get().is_some() {
                return Ok(());
            }
            if active {
                let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
                let bounds = monitor_bounds(monitor).ok_or_else(Error::from_thread)?;
                let mut placement = WINDOWPLACEMENT {
                    length: size_of::<WINDOWPLACEMENT>() as u32,
                    ..Default::default()
                };
                GetWindowPlacement(hwnd, &mut placement)?;
                let saved = Windowed {
                    style,
                    placement,
                    visible: IsWindowVisible(hwnd).as_bool(),
                };
                // Set the state before SetWindowPos sends synchronous sizing and hit-test messages.
                self.fullscreen.set(Some(saved));
                let borderless =
                    (style & !(WS_OVERLAPPEDWINDOW | WS_MAXIMIZE | WS_MINIMIZE).0) | WS_POPUP.0;
                SetWindowLongPtrW(hwnd, GWL_STYLE, borderless as isize);
                if let Err(error) = fit_monitor(hwnd, bounds.rcMonitor) {
                    self.set_fullscreen(false)?;
                    return Err(error);
                }
            } else if let Some(saved) = self.fullscreen.take() {
                let mut placement = saved.placement;
                if !saved.visible {
                    placement.showCmd = SW_HIDE.0 as u32;
                }
                SetWindowLongPtrW(hwnd, GWL_STYLE, saved.style as isize);
                SetWindowPlacement(hwnd, &placement)?;
                SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOSIZE
                        | SWP_NOMOVE
                        | SWP_NOZORDER
                        | SWP_NOOWNERZORDER
                        | SWP_NOACTIVATE
                        | SWP_FRAMECHANGED,
                )?;
            }
        }
        Ok(())
    }
}
