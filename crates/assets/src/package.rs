//! Open v1 and v2 asset packages. New builds use independently authenticated v2 blocks.
mod chunks;
mod cook;
mod legacy;
mod reader;

use aes_gcm::{Aes256Gcm, KeyInit};
use chunks::{Blocks, Section};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    ops::Range,
    path::Path,
    sync::Arc,
};

pub use cook::{PackOptions, cook, cook_with_options};
pub use reader::AssetReader;
pub type Result<T> = std::result::Result<T, String>;
pub(crate) fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn valid_name(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

enum Format {
    Legacy(legacy::Legacy),
    Chunked {
        blocks: Arc<Blocks>,
        entries: BTreeMap<String, Range<u64>>,
    },
}
pub struct Package(Format);

impl Package {
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self> {
        let mut file = File::open(path).map_err(error)?;
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic).map_err(error)?;
        let format = match &magic {
            legacy::MAGIC => Format::Legacy(legacy::Legacy::open(file, key)?),
            chunks::MAGIC => {
                let blocks = Blocks::open(file, path, Aes256Gcm::new(key.into())).map_err(error)?;
                let entries = read_directory(&blocks).map_err(error)?;
                Format::Chunked { blocks, entries }
            }
            _ => return Err("Unrecognized RustyBox asset package".into()),
        };
        Ok(Self(format))
    }

    /// Look up the plaintext size without reading payload bytes.
    pub fn asset_len(&self, name: &str) -> Option<u64> {
        match &self.0 {
            Format::Legacy(old) => old.asset_len(name),
            Format::Chunked { entries, .. } => entries.get(name).map(|span| span.end - span.start),
        }
    }

    /// Open a seekable view. V1 authenticates the whole asset when the reader is opened.
    pub fn reader(&self, name: &str) -> Result<Option<AssetReader>> {
        match &self.0 {
            Format::Legacy(old) => Ok(old.read(name)?.map(AssetReader::legacy)),
            Format::Chunked { blocks, entries } => Ok(entries.get(name).map(|span| {
                AssetReader::blocks(
                    blocks.clone(),
                    Section::Data,
                    span.start,
                    span.end - span.start,
                )
            })),
        }
    }

    /// Load an entire asset into memory. Use reader() for large resources.
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        if let Format::Legacy(old) = &self.0 {
            return old.read(name);
        }
        self.reader(name)?.map(read_all).transpose()
    }
}

pub(crate) fn read_all(mut reader: AssetReader) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(error)?;
    Ok(bytes)
}

fn read_directory(blocks: &Arc<Blocks>) -> io::Result<BTreeMap<String, Range<u64>>> {
    let mut reader = blocks.directory();
    let mut remaining = reader.len();
    let mut offset = 0u64;
    let mut entries = BTreeMap::new();
    while remaining != 0 {
        let mut header = [0u8; 12];
        reader.read_exact(&mut header)?;
        let name_len = u32::from_le_bytes(header[..4].try_into().unwrap());
        let size = u64::from_le_bytes(header[4..].try_into().unwrap());
        remaining = remaining
            .checked_sub(12 + u64::from(name_len))
            .ok_or_else(|| chunks::invalid("Invalid package entry length"))?;
        let end = offset
            .checked_add(size)
            .filter(|&n| n <= blocks.footer.data_len)
            .ok_or_else(|| chunks::invalid("Asset exceeds the package payload"))?;
        let mut bytes = Vec::new();
        (&mut reader)
            .take(u64::from(name_len))
            .read_to_end(&mut bytes)?;
        let name =
            String::from_utf8(bytes).map_err(|_| chunks::invalid("Invalid UTF-8 asset name"))?;
        if !valid_name(&name) || entries.insert(name, offset..end).is_some() {
            return Err(chunks::invalid("Invalid or duplicate asset name"));
        }
        offset = end;
    }
    if offset != blocks.footer.data_len || !entries.contains_key("index.html") {
        return Err(chunks::invalid("Incomplete asset package"));
    }
    Ok(entries)
}
