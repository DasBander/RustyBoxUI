//! A native file or folder selection grants the host access to that location and its descendants.
use crate::Result;
use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::{Component, Path, PathBuf},
};
use windows::{
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::*,
        UI::Shell::*,
    },
    core::{HSTRING, PCWSTR},
};

#[derive(Clone)]
pub struct FileGrant {
    root: PathBuf,
}
impl FileGrant {
    /// Use from a native dialog, OS drop event, or a project location previously saved by the host.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            root: path.as_ref().canonicalize().map_err(|e| e.to_string())?,
        })
    }
    pub fn path(&self) -> &Path {
        &self.root
    }
    pub fn resolve(&self, relative: &Path) -> Result<PathBuf> {
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("Expected a relative project path".into());
        }
        let path = self
            .root
            .join(relative)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !path.starts_with(&self.root) {
            return Err("File is outside the selected location".into());
        }
        Ok(path)
    }
}
pub fn pick_folder(owner: HWND, title: &str) -> Result<Option<FileGrant>> {
    pick(owner, title, true)
}
pub fn pick_file(owner: HWND, title: &str) -> Result<Option<FileGrant>> {
    pick(owner, title, false)
}
/// Returns only a destination approved by the owner of the native Save As dialog.
pub fn pick_save_file(owner: HWND, suggested: &str, extension: &str) -> Result<Option<PathBuf>> {
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    if extension.is_empty() || !extension.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Invalid save extension".into());
    }
    if suggested.is_empty()
        || suggested.contains(['/', '\\', ':', '\0'])
        || suggested.chars().any(char::is_control)
    {
        return Err("Expected a filename".into());
    }
    let label = HSTRING::from(format!("{} files", extension.to_uppercase()));
    let pattern = HSTRING::from(format!("*.{extension}"));
    let error = |e: windows::core::Error| e.to_string();
    unsafe {
        let dialog: IFileSaveDialog =
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).map_err(error)?;
        dialog
            .SetOptions(
                dialog.GetOptions().map_err(error)?
                    | FOS_FORCEFILESYSTEM
                    | FOS_OVERWRITEPROMPT
                    | FOS_NOCHANGEDIR,
            )
            .map_err(error)?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: PCWSTR(label.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            }])
            .map_err(error)?;
        dialog
            .SetDefaultExtension(&HSTRING::from(extension))
            .map_err(error)?;
        dialog
            .SetFileName(&HSTRING::from(suggested))
            .map_err(error)?;
        if let Err(e) = dialog.Show(Some(owner)) {
            if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(e.to_string());
        }
        let raw = dialog
            .GetResult()
            .map_err(error)?
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(error)?;
        let path = PathBuf::from(OsString::from_wide(PCWSTR(raw.0).as_wide()));
        CoTaskMemFree(Some(raw.0.cast()));
        Ok(Some(path))
    }
}
fn pick(owner: HWND, title: &str, folder: bool) -> Result<Option<FileGrant>> {
    fn error(e: windows::core::Error) -> String {
        e.to_string()
    }
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(error)?;
        dialog.SetTitle(&HSTRING::from(title)).map_err(error)?;
        let flags = dialog.GetOptions().map_err(error)?
            | FOS_FORCEFILESYSTEM
            | FOS_NOCHANGEDIR
            | if folder {
                FOS_PICKFOLDERS
            } else {
                FOS_FILEMUSTEXIST
            };
        dialog.SetOptions(flags).map_err(error)?;
        if let Err(e) = dialog.Show(Some(owner)) {
            if e.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                return Ok(None);
            }
            return Err(error(e));
        }
        let p = dialog
            .GetResult()
            .map_err(error)?
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(error)?;
        let path = PathBuf::from(OsString::from_wide(PCWSTR(p.0).as_wide()));
        CoTaskMemFree(Some(p.0.cast()));
        FileGrant::from_path(path).map(Some)
    }
}
