//! The host owns file grants and paths. The page receives opaque IDs, image metadata and thumbnails.
use crate::{commands::Asset, image};
use rustybox_sdk::{Job, JobContext, Result, files::FileGrant};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Serialize, Deserialize, Clone)]
pub struct LocalImage {
    pub path: PathBuf,
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u32,
}
impl LocalImage {
    pub fn metadata(&self, root: Option<&Path>) -> Asset {
        let parent = self.path.parent().unwrap_or(Path::new(""));
        let location = root
            .and_then(|base| parent.strip_prefix(base).ok())
            .map(|p| p.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"))
            .unwrap_or_else(|| {
                parent
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
        let name = self
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Asset {
            id: self.id.clone(),
            title: self
                .path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            file_name: name,
            location,
            credit: "Local image".into(),
            thumbnail: None,
            width: self.width,
            height: self.height,
            bytes: self.bytes,
        }
    }
}
#[derive(Serialize, Deserialize, Default)]
pub struct Saved {
    pub folder: Option<PathBuf>,
    pub selected: String,
    pub images: Vec<LocalImage>,
    #[serde(default)]
    pub singles: Vec<PathBuf>,
}
impl Saved {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(b) => {
                serde_json::from_slice(&b).map_err(|e| format!("Cannot read Atlas project: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut tmp = NamedTempFile::new_in(path.parent().ok_or("Missing project directory")?)
            .map_err(err)?;
        serde_json::to_writer(&mut tmp, &self).map_err(err)?;
        tmp.flush().map_err(err)?;
        tmp.as_file().sync_all().map_err(err)?;
        tmp.persist(path).map_err(err)?;
        Ok(())
    }
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn read(path: &Path) -> Result<Vec<u8>> {
    let f = File::open(path).map_err(err)?;
    let mut bytes = vec![];
    f.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Image exceeds 64 MiB".into());
    }
    Ok(bytes)
}
pub fn inspect(path: &Path) -> Result<(LocalImage, String)> {
    let data = read(path)?;
    let (width, height) = image::dimensions(&data)?;
    let path = path.canonicalize().map_err(err)?;
    use std::os::windows::ffi::OsStrExt;
    let mut digest = Sha256::new();
    for u in path.as_os_str().encode_wide() {
        digest.update(u.to_le_bytes());
    }
    let id = format!("file-{:x}", digest.finalize());
    let entry = LocalImage {
        path,
        id,
        width,
        height,
        bytes: data.len() as u32,
    };
    Ok((entry, image::thumbnail(&data)?))
}
pub enum Update {
    Image(LocalImage, String),
    Skipped,
}
pub fn scan(folder: FileGrant) -> Result<Job<Update, ()>> {
    Job::spawn(move |ctx: JobContext<Update>| {
        let mut pending = vec![folder.path().to_owned()];
        let mut visited = HashSet::new();
        let mut imported = 0;
        while let Some(dir) = pending.pop() {
            ctx.checkpoint()?;
            if !visited.insert(dir.clone()) {
                continue;
            }
            let entries = match fs::read_dir(dir) {
                Ok(entries) => entries,
                Err(_) => {
                    ctx.report(Update::Skipped)?;
                    continue;
                }
            };
            for entry in entries {
                ctx.checkpoint()?;
                let Ok(entry) = entry else {
                    ctx.report(Update::Skipped)?;
                    continue;
                };
                let path = entry.path();
                let Ok(relative) = path.strip_prefix(folder.path()) else {
                    continue;
                };
                let Ok(path) = folder.resolve(relative) else {
                    continue;
                };
                if path.is_dir() {
                    pending.push(path);
                    continue;
                }
                let extension = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                if !matches!(extension.as_str(), "jpeg" | "jpg" | "png" | "bmp") {
                    continue;
                }
                match inspect(&path) {
                    Ok((entry, thumb)) => {
                        ctx.report(Update::Image(entry, thumb))?;
                        imported += 1;
                        if imported >= 20_000 {
                            return Err("Folder import limit is 20,000 images".into());
                        }
                    }
                    Err(_) => ctx.report(Update::Skipped)?,
                }
            }
        }
        Ok(())
    })
}
