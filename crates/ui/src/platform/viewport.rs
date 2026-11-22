//! Synchronize CSS layout with the native swap chain and recover from device loss.
use super::{FRAME_INTERVAL_MS, FRAME_TIMER, Session, err};
use crate::graphics::Graphics;
use rustybox_core::response;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller3;
use windows::Win32::{
    Foundation::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
use windows::core::Interface;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::Win32::Graphics::DirectComposition::IDCompositionVisual;

type Bounds = (i32, i32, u32, u32);
#[derive(Default)]
pub(super) struct Surface {
    graphics: RefCell<Option<Graphics>>,
    visual: RefCell<Option<IDCompositionVisual>>,
    geometry: Cell<Option<Bounds>>,
    recovery: Cell<Option<(Instant, u8)>>,
}
pub(super) fn slot_id(params: &Value) -> Result<u32, String> {
    match params.get("viewport") {
        None => Ok(0),
        Some(id) => id
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .ok_or_else(|| "Invalid viewport ID".into()),
    }
}
impl Session {
    fn surface(&self, id: u32) -> Option<Rc<Surface>> {
        self.surfaces.borrow().get(&id).cloned()
    }
    pub fn with_graphics<T>(
        &self,
        id: u32,
        f: impl FnOnce(&mut Graphics) -> Result<T, String>,
    ) -> Result<T, String> {
        let s = self
            .surface(id)
            .ok_or("Attach the graphics viewport first")?;
        let mut renderer = s
            .graphics
            .try_borrow_mut()
            .map_err(|_| "Viewport is busy")?;
        f(renderer.as_mut().ok_or("Viewport device unavailable")?)
    }

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
        // WebView2 also applies Windows text scaling and page zoom to CSS coordinates.
        let mut raster = f64::from(unsafe { GetDpiForWindow(self.hwnd) }.max(96)) / 96.;
        let mut zoom = 1.;
        if let Some(controller) = self.controller.borrow().as_ref() {
            unsafe {
                if let Ok(controller3) = controller.cast::<ICoreWebView2Controller3>() {
                    let _ = controller3.RasterizationScale(&mut raster);
                }
                let _ = controller.ZoomFactor(&mut zoom);
            }
        }
        let scale = raster * zoom;
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
        let id = slot_id(&p)?;
        let surface = match self.surface(id) {
            Some(surface) => surface,
            None => {
                if self.surfaces.borrow().len() >= 16 {
                    return Err("A window supports up to 16 viewports".into());
                }
                let surface = Rc::new(Surface::default());
                self.surfaces.borrow_mut().insert(id, surface.clone());
                surface
            }
        };
        if surface.geometry.get() != Some(bounds) || surface.graphics.borrow().is_none() {
            if let Err(error) = self.ensure_surface(&surface, bounds) {
                self.detach_surface(id);
                return Err(error);
            }
            if unsafe { SetTimer(Some(self.hwnd), FRAME_TIMER, FRAME_INTERVAL_MS, None) } == 0 {
                self.detach_surface(id);
                return Err("Could not start viewport frame timer".into());
            }
            surface.geometry.set(Some(bounds));
            surface.recovery.set(None);
            self.present_surface(id, &surface);
        }
        let mut info = self.graphics_info(id);
        if stale {
            info["layoutPending"] = true.into();
        }
        Ok(info)
    }
    pub(super) fn graphics_info(&self, id: u32) -> Value {
        let Some(surface) = self.surface(id) else {
            return json!({"active":false,"viewport":id});
        };
        let graphics = surface.graphics.borrow();
        let Some(graphics) = graphics.as_ref() else {
            return json!({"active":false,"viewport":id});
        };
        let mut info = graphics.info();
        info["viewport"] = id.into();
        if let Some((x, y, width, height)) = surface.geometry.get() {
            info["bounds"] = json!({ "x":x,"y":y,"width":width,"height":height });
        }
        info
    }
    pub(super) fn detach_surface(&self, id: u32) {
        let surface = self.surfaces.borrow_mut().remove(&id);
        if let Some(surface) = surface {
            surface.graphics.borrow_mut().take();
            if let (Some(comp), Some(visual)) =
                (&self.composition, surface.visual.borrow_mut().take())
            {
                comp.remove_surface(&visual);
            }
        }
        if self.surfaces.borrow().is_empty() {
            unsafe {
                let _ = KillTimer(Some(self.hwnd), FRAME_TIMER);
            }
        }
    }
    fn ensure_surface(&self, s: &Surface, (x, y, w, h): Bounds) -> Result<(), String> {
        if s.visual.borrow().is_none()
            && let Some(comp) = &self.composition
        {
            *s.visual.borrow_mut() = Some(comp.create_surface()?);
        }
        let mut graphics = s.graphics.borrow_mut();
        if graphics.is_none() {
            *graphics = Some(Graphics::load(
                self.hwnd.0 as usize,
                s.visual.borrow().as_ref(),
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
        for surface in self.surfaces.borrow().values() {
            surface.graphics.borrow_mut().take();
            surface.recovery.set(Some((Instant::now(), 0)));
        }
    }
    pub fn wait_ms(&self, maximum: u32) -> u32 {
        if self.surfaces.borrow().is_empty() {
            maximum
        } else {
            maximum.min(FRAME_INTERVAL_MS)
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
        let slots = self.surfaces.borrow().clone();
        for (id, surface) in slots {
            if let Some((next, attempt)) = surface.recovery.get()
                && Instant::now() >= next
                && let Some(bounds) = surface.geometry.get()
            {
                match self.ensure_surface(&surface, bounds) {
                    Ok(()) => {
                        surface.recovery.set(None);
                        self.emit(json!({"event":"graphics_restored","viewport":id}));
                        let _ = self.post(
                            json!({"v":1,"event":"graphics-restored","data":{"viewport":id}}),
                        );
                    }
                    Err(error) if attempt >= 3 => {
                        self.detach_surface(id);
                        self.emit(json!({"event":"graphics_failed","viewport":id,"message":error}));
                    }
                    Err(_) => surface.recovery.set(Some((
                        Instant::now() + Duration::from_millis(250 << attempt),
                        attempt + 1,
                    ))),
                }
            }
        }
        self.present(false);
    }
    fn present_surface(&self, id: u32, surface: &Surface) {
        let error = {
            // Synchronous Win32 calls may reenter the window procedure during ResizeBuffers.
            let Ok(mut renderer) = surface.graphics.try_borrow_mut() else {
                return;
            };
            let Some(renderer) = renderer.as_mut() else {
                return;
            };
            renderer.frame(self.started.elapsed().as_secs_f32()).err()
        };
        if let Some(error) = error {
            surface.graphics.borrow_mut().take();
            surface.recovery.set(Some((Instant::now(), 0)));
            self.emit(json!({"event":"graphics_lost","viewport":id,"message":error}));
        }
    }
    pub(super) fn present(&self, resized: bool) {
        if !resized
            && self.last_frame.get().elapsed() < Duration::from_millis(FRAME_INTERVAL_MS.into())
        {
            return;
        }
        self.last_frame.set(Instant::now());
        let Ok(slots) = self.surfaces.try_borrow() else {
            return;
        };
        let slots = slots.clone();
        for (id, surface) in slots {
            self.present_surface(id, &surface);
        }
    }
}
