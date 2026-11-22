//! Synchronize CSS layout with the native swap chain and recover from device loss.
use super::{FRAME_INTERVAL_MS, FRAME_TIMER, Session, err};
use crate::graphics::Graphics;
use rustybox_core::response;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

impl Session {
    pub(super) fn attach(&self, p: Value) -> Result<Value, String> {
        let dimension = |key: &str| -> Result<f64, String> {
            let n = p
                .get(key)
                .and_then(Value::as_f64)
                .ok_or_else(|| format!("Missing {key}"))?;
            if !n.is_finite() || !(0.0..=16384.0).contains(&n) {
                Err("Invalid surface geometry".into())
            } else {
                Ok(n)
            }
        };
        let scale = unsafe { GetDpiForWindow(self.hwnd) } as f64 / 96.;
        let left = dimension("x")?;
        let top = dimension("y")?;
        let x = (left * scale).round() as i32;
        let y = (top * scale).round() as i32;
        let mut right = ((left + dimension("width")?) * scale).round() as i32;
        let mut bottom = ((top + dimension("height")?) * scale).round() as i32;
        let mut client = RECT::default();
        unsafe {
            GetClientRect(self.hwnd, &mut client).map_err(err)?;
        }
        // Layout and the HWND advance independently during live sizing and DPI changes.
        let mut stale = false;
        if p.get("viewportWidth").is_some() || p.get("viewportHeight").is_some() {
            let tolerance = (scale * 0.5).ceil();
            let viewport_width = dimension("viewportWidth")? * scale;
            let viewport_height = dimension("viewportHeight")? * scale;
            if f64::from(right) > viewport_width + tolerance
                || f64::from(bottom) > viewport_height + tolerance
            {
                return Err("Native surface must fit inside the layout viewport".into());
            }
            stale = (viewport_width - f64::from(client.right)).abs() > tolerance
                || (viewport_height - f64::from(client.bottom)).abs() > tolerance;
            // Even the current layout can differ by a physical pixel because innerWidth is integral.
            right = right.min(client.right);
            bottom = bottom.min(client.bottom);
            if stale && (x >= right || y >= bottom) {
                return Ok(json!({"layoutPending":true}));
            }
        }
        if x >= right || y >= bottom || right > client.right || bottom > client.bottom {
            return Err("Native surface must fit inside the client area".into());
        }
        let bounds = (x, y, (right - x) as u32, (bottom - y) as u32);
        if self.geometry.get() != Some(bounds) || self.graphics.borrow().is_none() {
            self.ensure_surface(bounds)?;
            if self.geometry.get().is_none()
                && unsafe { SetTimer(Some(self.hwnd), FRAME_TIMER, FRAME_INTERVAL_MS, None) } == 0
            {
                self.detach_surface();
                return Err("Could not start the viewport frame timer".into());
            }
            self.geometry.set(Some(bounds));
            self.recovery.set(None);
            // ResizeBuffers discards the previous image. Fill the new buffer immediately.
            self.present(true);
        }
        let mut info = self.graphics_info();
        if stale {
            info["layoutPending"] = true.into();
        }
        Ok(info)
    }
    pub(super) fn graphics_info(&self) -> Value {
        let graphics = self.graphics.borrow();
        let Some(graphics) = graphics.as_ref() else {
            return json!({"active":false});
        };
        let mut info = graphics.info();
        if let Some((x, y, width, height)) = self.geometry.get() {
            info["bounds"] = json!({ "x": x, "y": y, "width": width, "height": height });
        }
        info
    }
    pub(super) fn detach_surface(&self) {
        if self.geometry.take().is_some() {
            unsafe {
                let _ = KillTimer(Some(self.hwnd), FRAME_TIMER);
            }
        }
        self.recovery.set(None);
        self.graphics.borrow_mut().take();
    }
    fn ensure_surface(&self, (x, y, w, h): (i32, i32, u32, u32)) -> Result<(), String> {
        let mut graphics = self.graphics.borrow_mut();
        if graphics.is_none() {
            *graphics = Some(Graphics::load(
                self.hwnd.0 as usize,
                self.composition.as_ref().map(|c| &c.surface),
                &self.graphics_path,
                w,
                h,
            )?);
        }
        graphics.as_ref().unwrap().resize(x, y, w, h)?;
        if let Some(comp) = &self.composition {
            comp.commit()?;
        }
        Ok(())
    }
    pub fn reset_graphics(&self) {
        if self.geometry.get().is_some() {
            self.graphics.borrow_mut().take();
            self.recovery.set(Some((Instant::now(), 0)));
        }
    }
    pub fn wait_ms(&self, maximum: u32) -> u32 {
        if self.graphics.borrow().is_some() || self.recovery.get().is_some() {
            maximum.min(16)
        } else {
            maximum
        }
    }
    pub fn fit(&self) {
        if let Some(controller) = self.controller.borrow().as_ref() {
            unsafe {
                let mut rect = RECT::default();
                if GetClientRect(self.hwnd, &mut rect).is_ok() {
                    let _ = controller.SetBounds(rect);
                }
            }
        }
    }
    pub fn tick(&self) {
        for (id, session, result) in self.storage.drain() {
            let _ = self.post(response(id, &session, result));
        }
        if unsafe { IsIconic(GetAncestor(self.hwnd, GA_ROOT)).as_bool() } {
            return;
        }
        if let Some((next, attempt)) = self.recovery.get()
            && Instant::now() >= next
            && let Some(bounds) = self.geometry.get()
        {
            match self.ensure_surface(bounds) {
                Ok(()) => {
                    self.recovery.set(None);
                    self.emit(json!({"event":"graphics_restored"}));
                    let _ = self.post(json!({"v":1,"event":"graphics-restored","data":{}}));
                }
                Err(error) if attempt >= 3 => {
                    self.detach_surface();
                    self.emit(json!({"event":"graphics_failed","message":error}));
                }
                Err(_) => self.recovery.set(Some((
                    Instant::now() + Duration::from_millis(250 << attempt),
                    attempt + 1,
                ))),
            }
        }
        self.present(false);
    }
    pub(super) fn present(&self, resized: bool) {
        if !resized
            && self.last_frame.get().elapsed() < Duration::from_millis(FRAME_INTERVAL_MS.into())
        {
            return;
        }
        let error = {
            // A synchronous Win32/COM call may have reentered our window procedure.
            let Ok(mut graphics) = self.graphics.try_borrow_mut() else {
                return;
            };
            let Some(g) = graphics.as_mut() else { return };
            self.last_frame.set(Instant::now());
            g.frame(self.started.elapsed().as_secs_f32()).err()
        };
        if let Some(error) = error {
            self.reset_graphics();
            self.emit(json!({"event":"graphics_lost","message":error}));
        }
    }
}
