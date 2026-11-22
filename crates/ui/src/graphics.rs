use libloading::Library;
use serde_json::{Value, json};
use std::{ffi::c_void, path::Path};
use windows::{Win32::Graphics::DirectComposition::IDCompositionVisual, core::Interface};
type Create = unsafe extern "C" fn(usize, u32, u32, *mut u64) -> i32;
type Resize = unsafe extern "C" fn(u64, i32, i32, u32, u32) -> i32;
type Frame = unsafe extern "C" fn(u64, f32) -> i32;
type Destroy = unsafe extern "C" fn(u64) -> i32;
type Hardware = unsafe extern "C" fn(u64) -> i32;
type CreateVisual = unsafe extern "C" fn(*mut c_void, u32, u32, *mut u64) -> i32;
type Device = unsafe extern "C" fn(u64, *mut *mut c_void) -> i32;
type Texture = unsafe extern "C" fn(u64, *mut c_void) -> i32;
type View = unsafe extern "C" fn(u64, f32, u32, u32) -> i32;
pub struct Graphics {
    handle: u64,
    resize: Resize,
    frame: Frame,
    destroy: Destroy,
    hardware: bool,
    device: Device,
    texture: Texture,
    set_view: View,
    has_texture: bool,
    composed: bool,
    frames: u64,
    _library: Library,
}
impl Graphics {
    pub fn load(
        parent: usize,
        visual: Option<&IDCompositionVisual>,
        path: &Path,
        w: u32,
        h: u32,
    ) -> Result<Self, String> {
        unsafe {
            let library =
                Library::new(path).map_err(|e| format!("Graphics extension unavailable: {e}"))?;
            let version = library
                .get::<unsafe extern "C" fn() -> u32>(b"rbg_abi_version\0")
                .map_err(|e| e.to_string())?;
            if version() != 1 {
                return Err("Graphics ABI mismatch".into());
            }
            let create: Create = *library.get(b"rbg_create\0").map_err(|e| e.to_string())?;
            let resize: Resize = *library.get(b"rbg_resize\0").map_err(|e| e.to_string())?;
            let frame: Frame = *library.get(b"rbg_frame\0").map_err(|e| e.to_string())?;
            let destroy: Destroy = *library.get(b"rbg_destroy\0").map_err(|e| e.to_string())?;
            let hardware: Hardware = *library.get(b"rbg_hardware\0").map_err(|e| e.to_string())?;
            let mut handle = 0;
            let create_visual: CreateVisual = *library
                .get(b"rbg_create_visual\0")
                .map_err(|e| e.to_string())?;
            let device: Device = *library
                .get(b"rbg_get_device\0")
                .map_err(|e| e.to_string())?;
            let texture: Texture = *library
                .get(b"rbg_set_texture\0")
                .map_err(|e| e.to_string())?;
            let set_view: View = *library.get(b"rbg_set_view\0").map_err(|e| e.to_string())?;
            let status = match visual {
                Some(visual) => create_visual(visual.as_raw(), w, h, &mut handle),
                None => create(parent, w, h, &mut handle),
            };
            if status != 0 {
                return Err("D3D11 device or shader initialization failed".into());
            }
            Ok(Self {
                handle,
                resize,
                frame,
                destroy,
                hardware: hardware(handle) == 1,
                device,
                texture,
                set_view,
                composed: visual.is_some(),
                has_texture: false,
                frames: 0,
                _library: library,
            })
        }
    }
    pub fn resize(&self, x: i32, y: i32, w: u32, h: u32) -> Result<(), String> {
        if unsafe { (self.resize)(self.handle, x, y, w, h) } != 0 {
            Err("Could not resize native surface".into())
        } else {
            Ok(())
        }
    }
    pub fn frame(&mut self, time: f32) -> Result<(), String> {
        if unsafe { (self.frame)(self.handle, time) } != 0 {
            Err("Direct3D presentation failed. Recreating the device.".into())
        } else {
            self.frames += 1;
            Ok(())
        }
    }
    pub fn device(&self, out: *mut *mut c_void) -> Result<(), String> {
        if unsafe { (self.device)(self.handle, out) } == 0 {
            Ok(())
        } else {
            Err("Cannot obtain D3D11 device".into())
        }
    }
    pub fn texture(&mut self, texture: *mut c_void) -> Result<(), String> {
        if unsafe { (self.texture)(self.handle, texture) } != 0 {
            return Err("Expected a shader-readable, single-sample RGBA8 or BGRA8 texture on the viewport device".into());
        }
        self.has_texture = !texture.is_null();
        Ok(())
    }
    pub fn options(&self, zoom: f32, channel: u32, motion: bool) -> Result<(), String> {
        if unsafe { (self.set_view)(self.handle, zoom, channel, u32::from(motion)) } == 0 {
            Ok(())
        } else {
            Err("Invalid texture view options".into())
        }
    }
    pub fn info(&self) -> Value {
        json!({
            "backend": "Direct3D 11",
            "hardware": self.hardware,
            "frames": self.frames,
            "active": true,
            "composed": self.composed,
            "source": if self.has_texture { "hostTexture" } else { "scene" },
        })
    }
}
impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            (self.destroy)(self.handle);
        }
    }
}
