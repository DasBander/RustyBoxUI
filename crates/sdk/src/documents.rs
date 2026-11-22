//! One user-selected destination per export. Chunks are committed only after the complete file arrives.
use crate::{Result, files};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};
use tempfile::NamedTempFile;
use windows::Win32::Foundation::HWND;

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase")]
pub enum SaveRequest {
    Begin { name: String, size: u64 },
    Write { id: u64, offset: u64, bytes: String },
    Finish { id: u64 },
    Cancel { id: u64 },
}

#[derive(Default)]
pub struct DocumentSaves {
    next: u64,
    active: Option<PendingSave>,
}
struct PendingSave {
    id: u64,
    size: u64,
    written: u64,
    destination: PathBuf,
    staging: NamedTempFile,
}

impl DocumentSaves {
    /// Register this on a trusted document window. The host chooses the permitted file extension.
    pub fn handle(&mut self, owner: HWND, extension: &str, request: SaveRequest) -> Result<Value> {
        match request {
            SaveRequest::Begin { name, size } => {
                if self.active.is_some() {
                    return Err("Another export is in progress".into());
                }
                let Some(path) = files::pick_save_file(owner, &name, extension)? else {
                    return Ok(Value::Null);
                };
                let parent = path.parent().ok_or("Output has no directory")?;
                let staging = NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
                self.next = self.next.checked_add(1).ok_or("Export ID exhausted")?;
                self.active = Some(PendingSave {
                    id: self.next,
                    size,
                    written: 0,
                    destination: path,
                    staging,
                });
                Ok(json!(self.next))
            }
            SaveRequest::Write { id, offset, bytes } => {
                let save = self.pending(id)?;
                let data = STANDARD.decode(bytes).map_err(|e| e.to_string())?;
                if offset != save.written || data.len() as u64 > save.size - save.written {
                    return Err("Unexpected export chunk".into());
                }
                save.staging.write_all(&data).map_err(|e| e.to_string())?;
                save.written += data.len() as u64;
                Ok(Value::Null)
            }
            SaveRequest::Finish { id } => {
                let save = self.pending(id)?;
                if save.written != save.size {
                    return Err("The export is incomplete".into());
                }
                save.staging
                    .as_file()
                    .sync_all()
                    .map_err(|e| e.to_string())?;
                let save = self.active.take().expect("validated export");
                save.staging
                    .persist(&save.destination)
                    .map_err(|e| e.error.to_string())?;
                Ok(Value::Null)
            }
            SaveRequest::Cancel { id } => {
                self.pending(id)?;
                self.active.take();
                Ok(Value::Null)
            }
        }
    }
    /// Discard a partial export when its originating page navigates away.
    pub fn cancel_pending(&mut self) {
        self.active.take();
    }

    fn pending(&mut self, id: u64) -> Result<&mut PendingSave> {
        self.active
            .as_mut()
            .filter(|save| save.id == id)
            .ok_or_else(|| "Unknown export".into())
    }
}
