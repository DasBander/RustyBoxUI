//! Stream source files into authenticated blocks and publish the index after the parts are durable.
use super::{
    Result,
    chunks::{self, Footer, Section, TAG},
    error, valid_name,
};
use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{AeadCore, OsRng},
};
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
};
use tempfile::{NamedTempFile, TempDir};

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
            if !valid_name(&relative) || u32::try_from(relative.len()).is_err() {
                return Err("Invalid asset path".into());
            }
            files.push(relative);
        }
    }
    Ok(())
}
/// Per-build resource settings, shared by the Rust API and the project manifest.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PackOptions {
    #[serde(rename = "chunkSizeKiB")]
    pub chunk_size_kib: u32,
    #[serde(rename = "partSizeMiB")]
    pub part_size_mib: Option<u64>,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            chunk_size_kib: 1024,
            part_size_mib: None,
        }
    }
}

impl PackOptions {
    fn footer(self) -> Result<Footer> {
        let chunk_size = self
            .chunk_size_kib
            .checked_mul(1024)
            .filter(|n| (chunks::MIN_CHUNK..=chunks::MAX_CHUNK).contains(n))
            .ok_or("resources.chunkSizeKiB must be between 4 and 16384")?;
        let chunks_per_part = match self.part_size_mib {
            None => 0,
            Some(mib) => mib.checked_mul(1024 * 1024)
                .map(|bytes| bytes / (u64::from(chunk_size) + TAG))
                .filter(|&n| n > 0)
                .ok_or("resources.partSizeMiB must fit at least one encrypted chunk including its 16-byte tag")?,
        };
        Ok(Footer {
            chunk_size,
            chunks_per_part,
            data_len: 0,
            directory_len: 0,
            part_id: Aes256Gcm::generate_nonce(&mut OsRng).into(),
        })
    }
}

/// Accumulate one plaintext block across consecutive writes, independent of file boundaries.
struct BlockWriter<'a, F> {
    cipher: &'a Aes256Gcm,
    section: Section,
    capacity: usize,
    pending: Vec<u8>,
    number: u64,
    length: u64,
    emit: F,
}

impl<'a, F: FnMut(u64, &[u8]) -> io::Result<()>> BlockWriter<'a, F> {
    fn new(cipher: &'a Aes256Gcm, section: Section, capacity: u32, emit: F) -> Self {
        Self {
            cipher,
            section,
            capacity: capacity as usize,
            pending: Vec::with_capacity(capacity as usize + TAG as usize),
            number: 0,
            length: 0,
            emit,
        }
    }

    fn emit_block(&mut self) -> io::Result<()> {
        chunks::encrypt(self.cipher, self.section, self.number, &mut self.pending)?;
        (self.emit)(self.number, &self.pending)?;
        self.number += 1;
        self.pending.clear();
        Ok(())
    }

    fn finish(mut self) -> io::Result<u64> {
        if !self.pending.is_empty() {
            self.emit_block()?;
        }
        Ok(self.length)
    }
}

impl<F: FnMut(u64, &[u8]) -> io::Result<()>> Write for BlockWriter<'_, F> {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let n = input.len().min(self.capacity - self.pending.len());
        self.length = self
            .length
            .checked_add(n as u64)
            .ok_or_else(|| chunks::invalid("Package size overflow"))?;
        self.pending.extend_from_slice(&input[..n]);
        if self.pending.len() == self.capacity {
            self.emit_block()?;
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct PartWriter {
    directory: TempDir,
    chunks_per_part: u64,
    current: Option<File>,
}

impl PartWriter {
    fn new(output: &Path, parent: &Path, footer: &Footer) -> io::Result<Self> {
        let path = footer.parts_path(output)?;
        // The random generation in the directory name keeps already-open packages readable.
        let directory = tempfile::Builder::new()
            .prefix(path.file_name().unwrap())
            .rand_bytes(0)
            .tempdir_in(parent)?;
        Ok(Self {
            directory,
            chunks_per_part: footer.chunks_per_part,
            current: None,
        })
    }

    fn write(&mut self, number: u64, bytes: &[u8]) -> io::Result<()> {
        if number.is_multiple_of(self.chunks_per_part) {
            self.finish_file()?;
            let path = self
                .directory
                .path()
                .join(chunks::part_name(number / self.chunks_per_part));
            self.current = Some(File::create_new(path)?);
        }
        self.current.as_mut().unwrap().write_all(bytes)
    }

    fn finish_file(&mut self) -> io::Result<()> {
        if let Some(file) = self.current.take() {
            file.sync_all()?;
        }
        Ok(())
    }
}

/// Seal a web root into a single RBPAK v2 with the default block size.
pub fn cook(root: &Path, output: &Path) -> Result<[u8; 32]> {
    cook_with_options(root, output, PackOptions::default())
}

/// Optionally split the encrypted payload into sibling part files. Publish the main package last.
pub fn cook_with_options(root: &Path, output: &Path, options: PackOptions) -> Result<[u8; 32]> {
    let mut footer = options.footer()?;
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

    let mut names = Vec::new();
    collect(&root, &root, &mut names)?;
    names.sort_unstable();
    if !names.iter().any(|n| n == "index.html") {
        return Err("Web root needs index.html".into());
    }

    let key = Aes256Gcm::generate_key(&mut OsRng);
    let cipher = Aes256Gcm::new(&key);
    let mut main = NamedTempFile::new_in(&parent).map_err(error)?;
    main.write_all(chunks::MAGIC).map_err(error)?;
    let mut parts = if footer.chunks_per_part > 0 {
        Some(PartWriter::new(output, &parent, &footer).map_err(error)?)
    } else {
        None
    };
    let mut data =
        BlockWriter::new(
            &cipher,
            Section::Data,
            footer.chunk_size,
            |i, bytes| match &mut parts {
                Some(parts) => parts.write(i, bytes),
                None => main.write_all(bytes),
            },
        );
    let mut entries = Vec::with_capacity(names.len());
    for name in names {
        let mut file = File::open(root.join(&name)).map_err(|e| format!("{name}: {e}"))?;
        let length = io::copy(&mut file, &mut data).map_err(|e| format!("{name}: {e}"))?;
        entries.push((name, length));
    }
    footer.data_len = data.finish().map_err(error)?;
    if let Some(parts) = &mut parts {
        parts.finish_file().map_err(error)?;
    }

    let mut directory = BlockWriter::new(
        &cipher,
        Section::Directory,
        footer.chunk_size,
        |_, bytes| main.write_all(bytes),
    );
    for (name, length) in entries {
        directory
            .write_all(&(name.len() as u32).to_le_bytes())
            .map_err(error)?;
        directory.write_all(&length.to_le_bytes()).map_err(error)?;
        directory.write_all(name.as_bytes()).map_err(error)?;
    }
    footer.directory_len = directory.finish().map_err(error)?;
    main.write_all(&footer.encode(&cipher).map_err(error)?)
        .map_err(error)?;
    main.as_file().sync_all().map_err(error)?;
    main.persist(output).map_err(error)?;
    if let Some(parts) = parts {
        let _ = parts.directory.keep();
    }
    Ok(key.into())
}
