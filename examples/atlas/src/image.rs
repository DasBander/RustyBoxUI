//! Decode bounded images on the CPU and upload on the selected viewport's owning D3D device.
use rustybox_sdk::{Result, View};
use std::io::Cursor;
use windows::Win32::Graphics::{Direct3D11::*, Dxgi::Common::*};

pub struct Pixels {
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
}
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Choose an image smaller than 64 MiB".into());
    }
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(error)?;
    let (w, h) = reader.into_dimensions().map_err(error)?;
    if w == 0 || h == 0 || w > 16384 || h > 16384 || u64::from(w) * u64::from(h) > 16_777_216 {
        return Err("Texture exceeds the 16-megapixel preview limit".into());
    }
    Ok((w, h))
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn decode(bytes: &[u8]) -> Result<Pixels> {
    let (width, height) = dimensions(bytes)?;
    let image = image::load_from_memory(bytes).map_err(error)?.into_rgba8();
    Ok(Pixels {
        width,
        height,
        rgba: image.into_raw(),
    })
}
pub fn thumbnail(bytes: &[u8]) -> Result<String> {
    use base64::Engine;
    dimensions(bytes)?;
    let image = image::load_from_memory(bytes)
        .map_err(error)?
        .thumbnail(88, 64)
        .to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 65)
        .encode_image(&image)
        .map_err(error)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(jpeg)
    ))
}
impl Pixels {
    pub fn display(&self, view: &View, slot: u32) -> Result<()> {
        let device = view.device_at(slot)?;
        let desc = D3D11_TEXTURE2D_DESC {
            Width: self.width,
            Height: self.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            ..Default::default()
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: self.rgba.as_ptr().cast(),
            SysMemPitch: self.width * 4,
            SysMemSlicePitch: 0,
        };
        let mut texture = None;
        unsafe {
            device
                .CreateTexture2D(&desc, Some(&data), Some(&mut texture))
                .map_err(error)?;
        }
        view.set_texture_at(slot, texture.as_ref())
    }
}
