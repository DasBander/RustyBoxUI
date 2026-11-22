//! RBPAK v1 stores encrypted entries followed by an encrypted directory and its length.
//! Every build has a new key. Nonce zero belongs to the directory, followed by one nonce per entry.
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng, Payload},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Mutex,
};
use tempfile::NamedTempFile;

pub type Result<T> = std::result::Result<T, String>;
const MAGIC: &[u8; 8] = b"RBPAK\x01\r\n";
const LIMIT: u64 = 128 * 1024 * 1024;
const MAX_INDEX: u64 = 8 * 1024 * 1024;
pub(crate) fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn valid_name(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 2048
        && !path.contains(['\\', ':', '\0'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
fn nonce(id: u64) -> Nonce<aes_gcm::aead::consts::U12> {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&id.to_be_bytes());
    bytes.into()
}
#[derive(Serialize, Deserialize)]
struct Entry {
    name: String,
    start: u64,
    size: u64,
}
pub struct Package {
    file: Mutex<File>,
    // The expanded key is large. Keep the shared asset handle small for windowed hosts.
    cipher: Box<Aes256Gcm>,
    entries: BTreeMap<String, (u64, Entry)>,
}
impl Package {
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self> {
        let mut file = File::open(path).map_err(error)?;
        let end = file.metadata().map_err(error)?.len();
        let mut bytes = [0u8; 8];
        file.read_exact(&mut bytes).map_err(error)?;
        if &bytes != MAGIC {
            return Err("Unrecognized RustyBox asset package".into());
        }
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
            if !valid_name(&entry.name)
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
fn collect(root: &Path, folder: &Path, files: &mut Vec<String>) -> Result<()> {
    for item in fs::read_dir(folder).map_err(error)? {
        let item = item.map_err(error)?;
        let name = item
            .file_name()
            .into_string()
            .map_err(|_| "Assets need UTF-8 filenames")?;
        if name.starts_with('.')
            || name == "node_modules"
            || name.ends_with(".map")
            || name.ends_with(".d.ts")
        {
            continue;
        }
        let path = item.path();
        let kind = item.file_type().map_err(error)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if item.metadata().map_err(error)?.file_attributes() & 0x400 != 0 {
                return Err("Linked assets are not supported".into());
            }
        }
        if kind.is_symlink() || !path.canonicalize().map_err(error)?.starts_with(root) {
            return Err("Linked assets are not supported".into());
        }
        if kind.is_dir() {
            collect(root, &path, files)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(error)?
                .to_str()
                .ok_or("Invalid asset path")?
                .replace('\\', "/");
            if !valid_name(&relative) || files.len() >= 65_535 {
                return Err("Invalid asset path or too many assets".into());
            }
            files.push(relative);
        }
    }
    Ok(())
}
/// Cook a web root into a new package. The caller binds the returned key to its executable.
pub fn cook(root: &Path, output: &Path) -> Result<[u8; 32]> {
    let root = root.canonicalize().map_err(error)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(error)?;
    if parent.starts_with(&root) {
        return Err("Write the package outside its input tree".into());
    }
    let mut names = vec![];
    collect(&root, &root, &mut names)?;
    names.sort();
    if !names.iter().any(|n| n == "index.html") {
        return Err("Web root needs index.html".into());
    }
    let key = Aes256Gcm::generate_key(&mut OsRng);
    let cipher = Aes256Gcm::new(&key);
    let mut output_file = NamedTempFile::new_in(parent).map_err(error)?;
    output_file.write_all(MAGIC).map_err(error)?;
    let mut entries = Vec::new();
    for (i, name) in names.into_iter().enumerate() {
        let file = File::open(root.join(&name)).map_err(error)?;
        let mut plain = Vec::new();
        file.take(LIMIT + 1)
            .read_to_end(&mut plain)
            .map_err(error)?;
        if plain.len() as u64 > LIMIT {
            return Err(format!("{name} exceeds the 128 MiB asset limit"));
        }
        let data = cipher
            .encrypt(
                &nonce(i as u64 + 1),
                Payload {
                    msg: &plain,
                    aad: name.as_bytes(),
                },
            )
            .map_err(error)?;
        let start = output_file.stream_position().map_err(error)?;
        output_file.write_all(&data).map_err(error)?;
        entries.push(Entry {
            name,
            start,
            size: data.len() as u64,
        });
    }
    let index = serde_json::to_vec(&entries).map_err(error)?;
    let directory = cipher
        .encrypt(
            &nonce(0),
            Payload {
                msg: &index,
                aad: MAGIC,
            },
        )
        .map_err(error)?;
    if directory.len() as u64 > MAX_INDEX {
        return Err("Asset directory too large".into());
    }
    output_file.write_all(&directory).map_err(error)?;
    output_file
        .write_all(&(directory.len() as u64).to_le_bytes())
        .map_err(error)?;
    output_file.as_file().sync_all().map_err(error)?;
    output_file.persist(output).map_err(error)?;
    Ok(key.into())
}
