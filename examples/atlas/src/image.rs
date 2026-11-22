use rustybox_sdk::{Result, View};
use std::path::{Path, PathBuf};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Direct3D11::*, Dxgi::Common::*, Imaging::*},
        System::Com::*,
        UI::Shell::{Common::COMDLG_FILTERSPEC, *},
    },
    core::{HSTRING, PCWSTR, w},
};
pub fn pick(owner: HWND) -> Result<Option<PathBuf>> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(error)?;
        dialog.SetTitle(w!("Import a texture")).map_err(error)?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("Images (PNG, JPEG, BMP)"),
                pszSpec: w!("*.png;*.jpg;*.jpeg;*.bmp"),
            }])
            .map_err(error)?;
        dialog
            .SetOptions(
                dialog.GetOptions().map_err(error)?
                    | FOS_FORCEFILESYSTEM
                    | FOS_FILEMUSTEXIST
                    | FOS_NOCHANGEDIR,
            )
            .map_err(error)?;
        if let Err(e) = dialog.Show(Some(owner)) {
            if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(error(e));
        }
        let path = dialog
            .GetResult()
            .map_err(error)?
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(error)?;
        use std::os::windows::ffi::OsStringExt;
        let value = std::ffi::OsString::from_wide(PCWSTR(path.0).as_wide());
        CoTaskMemFree(Some(path.0.cast()));
        Ok(Some(value.into()))
    }
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
}
pub struct ImageFile {
    factory: IWICImagingFactory,
    frame: IWICBitmapFrameDecode,
    pub width: u32,
    pub height: u32,
    pub bytes: u32,
}
impl ImageFile {
    pub fn open(path: &Path) -> Result<Self> {
        let bytes = std::fs::metadata(path).map_err(error)?.len();
        if bytes > 64 * 1024 * 1024 {
            return Err("Choose an image smaller than 64 MiB".into());
        }
        unsafe {
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                    .map_err(error)?;
            let decoder = factory
                .CreateDecoderFromFilename(
                    &HSTRING::from(path.as_os_str()),
                    None,
                    GENERIC_READ,
                    WICDecodeMetadataCacheOnDemand,
                )
                .map_err(error)?;
            let format = decoder.GetContainerFormat().map_err(error)?;
            if ![
                GUID_ContainerFormatPng,
                GUID_ContainerFormatJpeg,
                GUID_ContainerFormatBmp,
            ]
            .contains(&format)
            {
                return Err("Choose a PNG, JPEG, or BMP image".into());
            }
            let frame = decoder.GetFrame(0).map_err(error)?;
            let (mut width, mut height) = (0, 0);
            frame.GetSize(&mut width, &mut height).map_err(error)?;
            if width == 0
                || height == 0
                || width > 16384
                || height > 16384
                || u64::from(width) * u64::from(height) > 16_777_216
            {
                return Err("Texture exceeds the 16-megapixel preview limit".into());
            }
            Ok(Self {
                factory,
                frame,
                width,
                height,
                bytes: bytes as u32,
            })
        }
    }
    pub fn decode(&self) -> Result<Pixels> {
        unsafe {
            let converter = self.factory.CreateFormatConverter().map_err(error)?;
            converter
                .Initialize(
                    &self.frame,
                    &GUID_WICPixelFormat32bppRGBA,
                    WICBitmapDitherTypeNone,
                    None,
                    0.0,
                    WICBitmapPaletteTypeCustom,
                )
                .map_err(error)?;
            let stride = self.width * 4;
            let mut rgba = vec![0u8; (stride * self.height) as usize];
            converter
                .CopyPixels(std::ptr::null(), stride, &mut rgba)
                .map_err(error)?;
            Ok(Pixels {
                width: self.width,
                height: self.height,
                rgba,
            })
        }
    }
}
impl Pixels {
    pub fn display(&self, view: &View) -> Result<()> {
        let device = view.device()?;
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
        view.set_texture(texture.as_ref())
    }
}
