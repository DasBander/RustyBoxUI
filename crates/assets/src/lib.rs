//! Read development files or authenticate assets from an executable-bound release package.
mod binding;
mod package;
pub use binding::bind;
pub use package::{Package, Result, cook, valid_name};
use std::path::{Path, PathBuf};

pub enum Assets {
    Directory(PathBuf),
    Packed(Package),
}
impl Assets {
    pub fn open(development_root: &Path) -> Result<Self> {
        if let Some((name, key)) = binding::current()? {
            let exe = std::env::current_exe().map_err(package::error)?;
            return Package::open(&exe.with_file_name(name), &key).map(Self::Packed);
        }
        let root = development_root.canonicalize().map_err(package::error)?;
        if !root.join("index.html").is_file() {
            return Err(format!("index.html missing in {}", root.display()));
        }
        Ok(Self::Directory(root))
    }
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        if !valid_name(name) {
            return Err("Invalid asset name".into());
        }
        match self {
            Self::Packed(package) => package.read(name),
            Self::Directory(root) => {
                let path = match root.join(name).canonicalize() {
                    Ok(p) if p.starts_with(root) => p,
                    Ok(_) => return Err("Asset outside root".into()),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(package::error(e)),
                };
                std::fs::read(path).map(Some).map_err(package::error)
            }
        }
    }
}
