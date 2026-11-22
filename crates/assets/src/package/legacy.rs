//! Read packages produced through 0.5. V1 authenticates an entire entry at once.
use super::{Result, error, valid_name};
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    sync::Mutex,
};
pub(super) const MAGIC: &[u8; 8] = b"RBPAK\x01\r\n";
const LIMIT: u64 = 128 * 1024 * 1024;
const MAX_INDEX: u64 = 8 * 1024 * 1024;

fn nonce(id: u64) -> Nonce<aes_gcm::aead::consts::U12> {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&id.to_be_bytes());
    bytes.into()
}
#[derive(Deserialize)]
struct Entry {
    name: String,
    start: u64,
    size: u64,
}
pub(super) struct Legacy {
    file: Mutex<File>,
    // The expanded key is large. Keep the shared asset handle small for windowed hosts.
    cipher: Box<Aes256Gcm>,
    entries: BTreeMap<String, (u64, Entry)>,
}
impl Legacy {
    pub fn open(mut file: File, key: &[u8; 32]) -> Result<Self> {
        let end = file.metadata().map_err(error)?.len();
        let mut bytes = [0u8; 8];
        file.seek(SeekFrom::End(-8)).map_err(error)?;
        file.read_exact(&mut bytes).map_err(error)?;
        let size = u64::from_le_bytes(bytes);
        if !(16..=MAX_INDEX).contains(&size) || size + 16 > end {
            return Err("Invalid package directory".into());
        }
        let index_start = end - 8 - size;
        let mut data = vec![0; size as usize];
        file.seek(SeekFrom::Start(index_start)).map_err(error)?;
        file.read_exact(&mut data).map_err(error)?;
        let cipher = Aes256Gcm::new(key.into());
        let index = cipher
            .decrypt(
                &nonce(0),
                Payload {
                    msg: &data,
                    aad: MAGIC,
                },
            )
            .map_err(|_| "Asset directory authentication failed")?;
        let list: Vec<Entry> = serde_json::from_slice(&index).map_err(error)?;
        if list.len() > 65_535 {
            return Err("Too many package entries".into());
        }
        let mut entries = BTreeMap::new();
        let mut expected = 8u64;
        for (i, entry) in list.into_iter().enumerate() {
            if entry.name.len() > 2048
                || !valid_name(&entry.name)
                || entry.start != expected
                || !(16..=LIMIT + 16).contains(&entry.size)
            {
                return Err("Invalid asset entry".into());
            }
            expected = expected
                .checked_add(entry.size)
                .ok_or("Invalid asset length")?;
            if entries
                .insert(entry.name.clone(), (i as u64 + 1, entry))
                .is_some()
            {
                return Err("Duplicate asset".into());
            }
        }
        if expected != index_start || !entries.contains_key("index.html") {
            return Err("Incomplete asset package".into());
        }
        Ok(Self {
            file: Mutex::new(file),
            cipher: Box::new(cipher),
            entries,
        })
    }
    pub fn asset_len(&self, name: &str) -> Option<u64> {
        self.entries.get(name).map(|(_, entry)| entry.size - 16)
    }
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some((id, entry)) = self.entries.get(name) else {
            return Ok(None);
        };
        let mut encrypted = vec![0; entry.size as usize];
        {
            let mut file = self.file.lock().map_err(|_| "Asset reader unavailable")?;
            file.seek(SeekFrom::Start(entry.start)).map_err(error)?;
            file.read_exact(&mut encrypted).map_err(error)?;
        }
        self.cipher
            .decrypt(
                &nonce(*id),
                Payload {
                    msg: &encrypted,
                    aad: name.as_bytes(),
                },
            )
            .map(Some)
            .map_err(|_| "Asset authentication failed".into())
    }
}
