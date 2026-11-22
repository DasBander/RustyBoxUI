//! Optional D3D11 renderer. Every handle is confined to its creating thread.
#![cfg(windows)]
use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicU64, Ordering},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{
            Direct3D::Fxc::*, Direct3D::*, Direct3D11::*, DirectComposition::IDCompositionVisual,
            Dxgi::Common::*, Dxgi::*,
        },
        UI::WindowsAndMessaging::*,
    },
    core::{IUnknown, Interface, s, w},
};
thread_local! { static VIEWS: RefCell<HashMap<u64, Surface>> = RefCell::new(HashMap::new()); }
static NEXT: AtomicU64 = AtomicU64::new(1);
enum Output {
    Window(HWND),
    Visual(IDCompositionVisual),
}
struct Surface {
    output: Output,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swapchain: IDXGISwapChain,
    target: Option<ID3D11RenderTargetView>,
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    constants: ID3D11Buffer,
    texture_shader: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    texture: Option<ID3D11ShaderResourceView>,
    texture_size: [f32; 2],
    zoom: f32,
    channel: u32,
    motion: bool,
    width: u32,
    height: u32,
    hardware: bool,
}
fn guarded(f: impl FnOnce() -> windows::core::Result<()>) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(())) => 0,
        _ => -3,
    }
}
fn missing() -> windows::core::Error {
    windows::core::Error::from(E_INVALIDARG)
}
fn target(
    device: &ID3D11Device,
    swap: &IDXGISwapChain,
) -> windows::core::Result<ID3D11RenderTargetView> {
    unsafe {
        let back: ID3D11Texture2D = swap.GetBuffer(0)?;
        let mut view = None;
        device.CreateRenderTargetView(&back, None, Some(&mut view))?;
        view.ok_or_else(missing)
    }
}
fn compile(
    entry: windows::core::PCSTR,
    profile: windows::core::PCSTR,
) -> windows::core::Result<Vec<u8>> {
    let source = include_bytes!("scene.hlsl");
    let mut blob = None;
    let mut errors = None;
    unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            s!("RustyBox scene"),
            None,
            None,
            entry,
            profile,
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut blob,
            Some(&mut errors),
        )?;
        let blob = blob.ok_or_else(missing)?;
        Ok(
            std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize())
                .to_vec(),
        )
    }
}
impl Surface {
    fn create(parent: usize, width: u32, height: u32) -> windows::core::Result<Self> {
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("RustyBox Direct3D 11"),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                width as i32,
                height as i32,
                Some(HWND(parent as *mut c_void)),
                None,
                None,
                None,
            )?;
            match Self::init(Output::Window(hwnd), width, height) {
                Ok(surface) => Ok(surface),
                Err(e) => {
                    let _ = DestroyWindow(hwnd);
                    Err(e)
                }
            }
        }
    }
    unsafe fn init(output: Output, width: u32, height: u32) -> windows::core::Result<Self> {
        unsafe {
            let create = |driver| -> windows::core::Result<_> {
                let mut device = None;
                let mut context = None;
                D3D11CreateDevice(
                    None,
                    driver,
                    HMODULE::default(),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&[D3D_FEATURE_LEVEL_11_0]),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    None,
                    Some(&mut context),
                )?;
                Ok((
                    device.ok_or_else(missing)?,
                    context.ok_or_else(missing)?,
                    driver == D3D_DRIVER_TYPE_HARDWARE,
                ))
            };
            let (device, context, hardware) =
                create(D3D_DRIVER_TYPE_HARDWARE).or_else(|_| create(D3D_DRIVER_TYPE_WARP))?;
            let factory: IDXGIFactory2 = device.cast::<IDXGIDevice>()?.GetAdapter()?.GetParent()?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                ..Default::default()
            };
            let chain = match &output {
                Output::Window(hwnd) => {
                    factory.CreateSwapChainForHwnd(&device, *hwnd, &desc, None, None)?
                }
                Output::Visual(_) => factory.CreateSwapChainForComposition(&device, &desc, None)?,
            };
            let swapchain: IDXGISwapChain = chain.cast()?;
            let target = Some(target(&device, &swapchain)?);
            let mut vertex = None;
            let mut pixel = None;
            let mut constants = None;
            device.CreateVertexShader(
                &compile(s!("vs_main"), s!("vs_5_0"))?,
                None,
                Some(&mut vertex),
            )?;
            device.CreatePixelShader(
                &compile(s!("ps_main"), s!("ps_5_0"))?,
                None,
                Some(&mut pixel),
            )?;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: 32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut constants),
            )?;
            let mut texture_shader = None;
            device.CreatePixelShader(
                &compile(s!("ps_texture"), s!("ps_5_0"))?,
                None,
                Some(&mut texture_shader),
            )?;
            let mut sampler = None;
            device.CreateSamplerState(
                &D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    ComparisonFunc: D3D11_COMPARISON_NEVER,
                    MaxLOD: f32::MAX,
                    ..Default::default()
                },
                Some(&mut sampler),
            )?;
            if let Output::Visual(visual) = &output {
                visual.SetContent(&swapchain)?;
            }
            Ok(Self {
                output,
                texture_shader: texture_shader.ok_or_else(missing)?,
                sampler: sampler.ok_or_else(missing)?,
                texture: None,
                texture_size: [1.0, 1.0],
                zoom: 1.0,
                channel: 0,
                motion: false,
                device,
                context,
                swapchain,
                target,
                vertex: vertex.ok_or_else(missing)?,
                pixel: pixel.ok_or_else(missing)?,
                constants: constants.ok_or_else(missing)?,
                width,
                height,
                hardware,
            })
        }
    }
    fn resize(&mut self, x: i32, y: i32, w: u32, h: u32) -> windows::core::Result<()> {
        unsafe {
            match &self.output {
                Output::Window(hwnd) => SetWindowPos(
                    *hwnd,
                    Some(HWND_TOP),
                    x,
                    y,
                    w as i32,
                    h as i32,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )?,
                Output::Visual(visual) => {
                    visual.SetOffsetX2(x as f32)?;
                    visual.SetOffsetY2(y as f32)?;
                }
            }
            if self.width != w || self.height != h {
                // DXGI requires all references to the old back buffer to be released before resizing.
                self.context.OMSetRenderTargets(None, None);
                self.target = None;
                self.swapchain.ResizeBuffers(
                    0,
                    w,
                    h,
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?;
                self.target = Some(target(&self.device, &self.swapchain)?);
                self.width = w;
                self.height = h;
            }
        }
        Ok(())
    }
    fn set_texture(&mut self, texture: Option<&ID3D11Texture2D>) -> windows::core::Result<()> {
        unsafe {
            if let Some(texture) = texture {
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                texture.GetDesc(&mut desc);
                if texture.GetDevice()?.cast::<IUnknown>()? != self.device.cast::<IUnknown>()?
                    || desc.ArraySize != 1
                    || desc.SampleDesc.Count != 1
                    || desc.BindFlags & D3D11_BIND_SHADER_RESOURCE.0 as u32 == 0
                    || !matches!(
                        desc.Format,
                        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM
                    )
                {
                    return Err(missing());
                }
                let mut view = None;
                self.device
                    .CreateShaderResourceView(texture, None, Some(&mut view))?;
                self.texture = view;
                self.texture_size = [desc.Width as f32, desc.Height as f32];
            } else {
                self.texture = None;
            }
        }
        Ok(())
    }
    fn frame(&self, time: f32) -> windows::core::Result<()> {
        unsafe {
            // A hidden or occluded swap chain can still accept Present after device removal.
            self.device.GetDeviceRemovedReason()?;
            if self.target.is_none() {
                return Err(missing());
            }
            let params = [
                time,
                self.width as f32,
                self.height as f32,
                self.zoom,
                self.texture_size[0],
                self.texture_size[1],
                self.channel as f32,
                f32::from(self.motion),
            ];
            self.context
                .UpdateSubresource(&self.constants, 0, None, params.as_ptr().cast(), 0, 0);
            self.context
                .OMSetRenderTargets(Some(std::slice::from_ref(&self.target)), None);
            self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: self.width as f32,
                Height: self.height as f32,
                MaxDepth: 1.,
                ..Default::default()
            }]));
            self.context
                .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.context.VSSetShader(&self.vertex, None);
            let shader = if self.texture.is_some() {
                &self.texture_shader
            } else {
                &self.pixel
            };
            self.context.PSSetShader(shader, None);
            self.context
                .PSSetShaderResources(0, Some(std::slice::from_ref(&self.texture)));
            self.context
                .PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            self.context
                .PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            self.context.Draw(3, 0);
            self.context.PSSetShaderResources(0, Some(&[None]));
            self.swapchain.Present(0, DXGI_PRESENT(0)).ok()
        }
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            self.context.ClearState();
            match &self.output {
                Output::Window(hwnd) => {
                    let _ = DestroyWindow(*hwnd);
                }
                Output::Visual(visual) => {
                    let _ = visual.SetContent(None::<&IUnknown>);
                }
            }
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn rbg_abi_version() -> u32 {
    1
}
/// # Safety
/// `out` must point to writable u64 storage; parent must be a live HWND on this thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rbg_create(parent: usize, width: u32, height: u32, out: *mut u64) -> i32 {
    if out.is_null() || parent == 0 || width == 0 || height == 0 || width > 16384 || height > 16384
    {
        return -1;
    }
    guarded(|| {
        let surface = Surface::create(parent, width, height)?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        VIEWS.with(|v| v.borrow_mut().insert(id, surface));
        unsafe {
            *out = id;
        }
        Ok(())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rbg_resize(id: u64, x: i32, y: i32, width: u32, height: u32) -> i32 {
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return -1;
    }
    guarded(|| {
        VIEWS.with(|v| {
            v.borrow_mut()
                .get_mut(&id)
                .ok_or_else(missing)?
                .resize(x, y, width, height)
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rbg_frame(id: u64, time: f32) -> i32 {
    if !time.is_finite() {
        return -1;
    }
    guarded(|| VIEWS.with(|v| v.borrow().get(&id).ok_or_else(missing)?.frame(time)))
}
#[unsafe(no_mangle)]
pub extern "C" fn rbg_hardware(id: u64) -> i32 {
    VIEWS.with(|v| v.borrow().get(&id).map_or(-2, |s| i32::from(s.hardware)))
}
#[unsafe(no_mangle)]
pub extern "C" fn rbg_destroy(id: u64) -> i32 {
    guarded(|| VIEWS.with(|v| v.borrow_mut().remove(&id).map(|_| ()).ok_or_else(missing)))
}

/// # Safety
/// visual is a live IDCompositionVisual on this thread. The DLL retains its own reference.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rbg_create_visual(
    visual: *mut c_void,
    width: u32,
    height: u32,
    out: *mut u64,
) -> i32 {
    if out.is_null()
        || visual.is_null()
        || !(1..=16384).contains(&width)
        || !(1..=16384).contains(&height)
    {
        return -1;
    }
    unsafe {
        *out = 0;
    }
    guarded(|| {
        let visual =
            unsafe { IDCompositionVisual::from_raw_borrowed(&visual) }.ok_or_else(missing)?;
        let surface = unsafe { Surface::init(Output::Visual(visual.clone()), width, height)? };
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        VIEWS.with(|views| views.borrow_mut().insert(id, surface));
        unsafe {
            *out = id;
        }
        Ok(())
    })
}
/// # Safety
/// out receives an owning ID3D11Device reference. The caller must Release it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rbg_get_device(id: u64, out: *mut *mut c_void) -> i32 {
    if out.is_null() {
        return -1;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    guarded(|| {
        VIEWS.with(|views| {
            let views = views.borrow();
            let surface = views.get(&id).ok_or_else(missing)?;
            unsafe {
                *out = surface.device.clone().into_raw();
            }
            Ok(())
        })
    })
}
/// # Safety
/// texture is null or a valid same-device ID3D11Texture2D. The caller retains ownership.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rbg_set_texture(id: u64, texture: *mut c_void) -> i32 {
    guarded(|| {
        VIEWS.with(|views| {
            views
                .borrow_mut()
                .get_mut(&id)
                .ok_or_else(missing)?
                .set_texture(unsafe { ID3D11Texture2D::from_raw_borrowed(&texture) })
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rbg_set_view(id: u64, zoom: f32, channel: u32, motion: u32) -> i32 {
    if !(0.25..=8.0).contains(&zoom) || channel > 4 || motion > 1 {
        return -1;
    }
    guarded(|| {
        VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            let surface = views.get_mut(&id).ok_or_else(missing)?;
            surface.zoom = zoom;
            surface.channel = channel;
            surface.motion = motion != 0;
            Ok(())
        })
    })
}
