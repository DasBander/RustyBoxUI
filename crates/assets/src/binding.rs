//! The offline content key travels in a Windows executable resource. Bind before code signing.
use super::package::{Result, error};
use std::path::Path;
use windows::{
    Win32::System::LibraryLoader::*,
    core::{HSTRING, PCWSTR, w},
};

const NAME: PCWSTR = w!("RUSTYBOX_ASSETS_V1");
const DATA: PCWSTR = PCWSTR(10 as *const u16); // RT_RCDATA
pub fn bind(executable: &Path, filename: &str, key: &[u8; 32]) -> Result<()> {
    if filename.len() > 2048
        || !super::valid_name(filename)
        || filename.contains('/')
        || !filename.ends_with(".rbpak")
    {
        return Err("Package must be an adjacent .rbpak file".into());
    }
    let mut descriptor = key.to_vec();
    descriptor.extend_from_slice(filename.as_bytes());
    unsafe {
        let update =
            BeginUpdateResourceW(&HSTRING::from(executable.as_os_str()), false).map_err(error)?;
        if let Err(e) = UpdateResourceW(
            update,
            DATA,
            NAME,
            0,
            Some(descriptor.as_ptr().cast()),
            descriptor.len() as u32,
        ) {
            let _ = EndUpdateResourceW(update, true);
            return Err(error(e));
        }
        EndUpdateResourceW(update, false).map_err(error)
    }
}
pub fn current() -> Result<Option<(String, [u8; 32])>> {
    unsafe {
        let module = GetModuleHandleW(None).map_err(error)?;
        let resource = FindResourceW(Some(module), NAME, DATA);
        if resource.is_invalid() {
            let error = windows::core::Error::from_thread();
            if matches!(error.code().0 as u32, 0x80070714..=0x80070717) {
                return Ok(None);
            }
            return Err(error.to_string());
        }
        let size = SizeofResource(Some(module), resource) as usize;
        if !(33..=2080).contains(&size) {
            return Err("Invalid asset binding".into());
        }
        let loaded = LoadResource(Some(module), resource).map_err(error)?;
        let address = LockResource(loaded);
        if address.is_null() {
            return Err("Missing asset binding".into());
        }
        let bytes = std::slice::from_raw_parts(address.cast::<u8>(), size);
        let name = std::str::from_utf8(&bytes[32..]).map_err(error)?;
        if !super::valid_name(name) || name.contains('/') || !name.ends_with(".rbpak") {
            return Err("Invalid bound package name".into());
        }
        Ok(Some((name.to_owned(), bytes[..32].try_into().unwrap())))
    }
}
