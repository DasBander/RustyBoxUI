use rustybox_sdk::{Result, View};
use serde::Deserialize;
use windows::{
    Win32::Graphics::{Direct3D::Fxc::*, Direct3D::*, Direct3D11::*, Dxgi::Common::*},
    core::{PCSTR, s},
};

#[derive(Clone, Deserialize)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub primary: [f32; 4],
    pub secondary: [f32; 4],
}

fn compile(source: &str, entry: PCSTR, profile: PCSTR) -> Result<ID3DBlob> {
    let (mut blob, mut errors) = (None, None);
    unsafe {
        let result = D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            s!("showcase"),
            None,
            None,
            entry,
            profile,
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut blob,
            Some(&mut errors),
        );
        if let Err(error) = result {
            return Err(errors.map_or_else(
                || error.to_string(),
                |message| {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        message.GetBufferPointer().cast(),
                        message.GetBufferSize(),
                    ))
                    .into_owned()
                },
            ));
        }
    }
    blob.ok_or("Empty shader".into())
}

unsafe fn bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
}

pub struct Scene {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    constants: ID3D11Buffer,
    target: Option<(ID3D11Texture2D, ID3D11RenderTargetView, u32, u32)>,
}

impl Scene {
    pub fn new(device: ID3D11Device, fullscreen: &str, shader: &str) -> Result<Self> {
        let vertex_code = compile(fullscreen, s!("vertex"), s!("vs_5_0"))?;
        let pixel_code = compile(
            &format!("{fullscreen}\n{shader}"),
            s!("pixel"),
            s!("ps_5_0"),
        )?;
        let create = || -> windows::core::Result<Self> {
            unsafe {
                let (mut vertex, mut pixel, mut constants) = (None, None, None);
                device.CreateVertexShader(bytes(&vertex_code), None, Some(&mut vertex))?;
                device.CreatePixelShader(bytes(&pixel_code), None, Some(&mut pixel))?;
                device.CreateBuffer(
                    &D3D11_BUFFER_DESC {
                        ByteWidth: 48,
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                        ..Default::default()
                    },
                    None,
                    Some(&mut constants),
                )?;
                Ok(Self {
                    context: device.GetImmediateContext()?,
                    device,
                    vertex: vertex.unwrap(),
                    pixel: pixel.unwrap(),
                    constants: constants.unwrap(),
                    target: None,
                })
            }
        };
        create().map_err(|e| e.to_string())
    }

    pub fn matches(&self, device: &ID3D11Device) -> bool {
        self.device == *device
    }

    pub fn draw(&mut self, view: &View, frame: &Frame) -> Result<()> {
        let (w, h) = (frame.width, frame.height);
        if w == 0
            || h == 0
            || w > 4096
            || h > 4096
            || u64::from(w) * u64::from(h) > 4_194_304
            || !frame
                .primary
                .iter()
                .chain(&frame.secondary)
                .all(|v| v.is_finite())
        {
            return Err("Invalid scene dimensions or parameters".into());
        }
        unsafe {
            if self.target.as_ref().is_none_or(|t| (t.2, t.3) != (w, h)) {
                let desc = D3D11_TEXTURE2D_DESC {
                    Width: w,
                    Height: h,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
                    ..Default::default()
                };
                let (mut texture, mut target) = (None, None);
                self.device
                    .CreateTexture2D(&desc, None, Some(&mut texture))
                    .map_err(|e| e.to_string())?;
                let texture = texture.unwrap();
                self.device
                    .CreateRenderTargetView(&texture, None, Some(&mut target))
                    .map_err(|e| e.to_string())?;
                view.set_texture(Some(&texture))?;
                self.target = Some((texture, target.unwrap(), w, h));
            }
            let constants = [[w as f32, h as f32, 0., 0.], frame.primary, frame.secondary];
            self.context.UpdateSubresource(
                &self.constants,
                0,
                None,
                constants.as_ptr().cast(),
                0,
                0,
            );
            self.context
                .OMSetRenderTargets(Some(&[Some(self.target.as_ref().unwrap().1.clone())]), None);
            self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: w as f32,
                Height: h as f32,
                MaxDepth: 1.,
                ..Default::default()
            }]));
            self.context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.VSSetShader(&self.vertex, None);
            self.context.PSSetShader(&self.pixel, None);
            self.context
                .PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            self.context.Draw(3, 0);
            self.context.OMSetRenderTargets(None, None);
        }
        Ok(())
    }
}
