//! V2 has two streams of authenticated blocks: asset bytes and a binary directory.
//! An authenticated, fixed-size footer describes both streams. Only asset blocks may live in parts.
use super::AssetReader;
use aes_gcm::{Aes256Gcm, Nonce, aead::AeadInPlace};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub(super) const MAGIC: &[u8; 8] = b"RBPAK\x02\r\n";
pub(super) const TAG: u64 = 16;
const FOOTER_PLAIN: usize = 40;
const FOOTER_SIZE: u64 = FOOTER_PLAIN as u64 + TAG;
pub(super) const MIN_CHUNK: u32 = 4 * 1024;
pub(super) const MAX_CHUNK: u32 = 16 * 1024 * 1024;

pub(super) fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[derive(Clone, Copy)]
pub(super) enum Section {
    Data = 1,
    Directory = 2,
    Footer = 3,
}

fn nonce(section: Section, number: u64) -> Nonce<aes_gcm::aead::consts::U12> {
    let mut bytes = [0; 12];
    bytes[..4].copy_from_slice(&(section as u32).to_be_bytes());
    bytes[4..].copy_from_slice(&number.to_be_bytes());
    bytes.into()
}

pub(super) fn encrypt(
    cipher: &Aes256Gcm,
    section: Section,
    number: u64,
    bytes: &mut Vec<u8>,
) -> io::Result<()> {
    cipher
        .encrypt_in_place(&nonce(section, number), MAGIC, bytes)
        .map_err(|_| invalid("Could not encrypt package block"))
}

fn decrypt(
    cipher: &Aes256Gcm,
    section: Section,
    number: u64,
    bytes: &mut Vec<u8>,
) -> io::Result<()> {
    cipher
        .decrypt_in_place(&nonce(section, number), MAGIC, bytes)
        .map_err(|_| invalid("Package block authentication failed"))
}

fn encoded_len(len: u64, chunk_size: u32) -> io::Result<u64> {
    let tags = len.div_ceil(u64::from(chunk_size)).checked_mul(TAG);
    tags.and_then(|n| n.checked_add(len))
        .ok_or_else(|| invalid("Package size overflow"))
}

pub(super) struct Footer {
    pub chunk_size: u32,
    pub chunks_per_part: u64,
    pub data_len: u64,
    pub directory_len: u64,
    pub part_id: [u8; 12],
}

impl Footer {
    pub fn encode(&self, cipher: &Aes256Gcm) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(FOOTER_SIZE as usize);
        bytes.extend(self.chunk_size.to_le_bytes());
        bytes.extend(self.chunks_per_part.to_le_bytes());
        bytes.extend(self.data_len.to_le_bytes());
        bytes.extend(self.directory_len.to_le_bytes());
        bytes.extend(self.part_id);
        encrypt(cipher, Section::Footer, 0, &mut bytes)?;
        Ok(bytes)
    }

    fn read(file: &mut File, cipher: &Aes256Gcm) -> io::Result<Self> {
        let file_len = file.metadata()?.len();
        file.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
        let mut bytes = vec![0; FOOTER_SIZE as usize];
        file.read_exact(&mut bytes)?;
        decrypt(cipher, Section::Footer, 0, &mut bytes)?;
        // The ciphertext length above fixes every field boundary, including the 16-byte tag.
        let footer = Self {
            chunk_size: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            chunks_per_part: u64::from_le_bytes(bytes[4..12].try_into().unwrap()),
            data_len: u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
            directory_len: u64::from_le_bytes(bytes[20..28].try_into().unwrap()),
            part_id: bytes[28..40].try_into().unwrap(),
        };
        if !(MIN_CHUNK..=MAX_CHUNK).contains(&footer.chunk_size)
            || !footer.chunk_size.is_multiple_of(1024)
        {
            return Err(invalid("Invalid package chunk size"));
        }
        let directory_offset = footer.directory_offset()?;
        let directory_len = encoded_len(footer.directory_len, footer.chunk_size)?;
        if directory_offset
            .checked_add(directory_len)
            .and_then(|n| n.checked_add(FOOTER_SIZE))
            != Some(file_len)
        {
            return Err(invalid("Package directory does not match the file length"));
        }
        Ok(footer)
    }

    fn directory_offset(&self) -> io::Result<u64> {
        let inline_bytes = if self.chunks_per_part == 0 {
            encoded_len(self.data_len, self.chunk_size)?
        } else {
            0
        };
        (MAGIC.len() as u64)
            .checked_add(inline_bytes)
            .ok_or_else(|| invalid("Package size overflow"))
    }

    pub fn parts_path(&self, package: &Path) -> io::Result<PathBuf> {
        use std::fmt::Write;
        let mut name = package
            .file_name()
            .ok_or_else(|| invalid("Package needs a filename"))?
            .to_os_string();
        let mut suffix = String::from(".chunks-");
        for byte in self.part_id {
            write!(suffix, "{byte:02x}").unwrap();
        }
        name.push(suffix);
        Ok(package.with_file_name(name))
    }
}

pub(super) fn part_name(number: u64) -> String {
    format!("{number:08}.rbchunk")
}

pub(super) struct Block {
    pub number: u64,
    pub bytes: Vec<u8>,
}

pub(super) struct Blocks {
    cipher: Aes256Gcm,
    pub footer: Footer,
    main: Mutex<File>,
    parts: Option<PathBuf>,
    // Limit open file handles even when a title has thousands of parts.
    last_part: Mutex<Option<(u64, File)>>,
    // Adjacent small assets often share a block. Concurrent responses can share its plaintext too.
    last_data: Mutex<Option<Arc<Block>>>,
}

impl Blocks {
    pub fn open(mut file: File, path: &Path, cipher: Aes256Gcm) -> io::Result<Arc<Self>> {
        let footer = Footer::read(&mut file, &cipher)?;
        let parts = (footer.chunks_per_part != 0)
            .then(|| footer.parts_path(path))
            .transpose()?;
        Ok(Arc::new(Self {
            cipher,
            footer,
            main: Mutex::new(file),
            parts,
            last_part: Mutex::new(None),
            last_data: Mutex::new(None),
        }))
    }

    pub fn directory(self: &Arc<Self>) -> AssetReader {
        AssetReader::blocks(
            self.clone(),
            Section::Directory,
            0,
            self.footer.directory_len,
        )
    }

    pub fn read_block(&self, section: Section, number: u64) -> io::Result<Arc<Block>> {
        if matches!(section, Section::Data) {
            let cached = self
                .last_data
                .lock()
                .map_err(|_| io::Error::other("Block cache unavailable"))?;
            if let Some(block) = cached.as_ref().filter(|b| b.number == number) {
                return Ok(block.clone());
            }
        }
        let (len, base) = match section {
            Section::Data => (self.footer.data_len, MAGIC.len() as u64),
            Section::Directory => (self.footer.directory_len, self.footer.directory_offset()?),
            Section::Footer => return Err(invalid("Invalid stream section")),
        };
        let chunk = u64::from(self.footer.chunk_size);
        let frame = chunk + TAG;
        let start = number
            .checked_mul(chunk)
            .filter(|&n| n < len)
            .ok_or_else(|| invalid("Invalid chunk number"))?;
        let size = (len - start).min(chunk) + TAG;
        let mut bytes = vec![0; size as usize];

        if let (Section::Data, Some(dir)) = (section, &self.parts) {
            let count = self.footer.chunks_per_part;
            let part = number / count;
            let local = number % count;
            let mut cached = self
                .last_part
                .lock()
                .map_err(|_| io::Error::other("Part reader unavailable"))?;
            if cached.as_ref().map(|(id, _)| *id) != Some(part) {
                let file = File::open(dir.join(part_name(part)))?;
                let first = number - local;
                let remaining = len - first * chunk;
                let part_len = remaining.min(count.saturating_mul(chunk));
                if file.metadata()?.len() != encoded_len(part_len, self.footer.chunk_size)? {
                    return Err(invalid("Package part has the wrong length"));
                }
                *cached = Some((part, file));
            }
            let file = &mut cached.as_mut().unwrap().1;
            file.seek(SeekFrom::Start(
                local
                    .checked_mul(frame)
                    .ok_or_else(|| invalid("Part offset overflow"))?,
            ))?;
            file.read_exact(&mut bytes)?;
        } else {
            let offset = number
                .checked_mul(frame)
                .and_then(|n| n.checked_add(base))
                .ok_or_else(|| invalid("Package offset overflow"))?;
            let mut file = self
                .main
                .lock()
                .map_err(|_| io::Error::other("Package reader unavailable"))?;
            file.seek(SeekFrom::Start(offset))?;
            file.read_exact(&mut bytes)?;
        }
        decrypt(&self.cipher, section, number, &mut bytes)?;
        let block = Arc::new(Block { number, bytes });
        if matches!(section, Section::Data) {
            *self
                .last_data
                .lock()
                .map_err(|_| io::Error::other("Block cache unavailable"))? = Some(block.clone());
        }
        Ok(block)
    }
}
