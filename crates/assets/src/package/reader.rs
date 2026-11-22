use super::chunks::{Block, Blocks, Section};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    ops::Range,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
enum Source {
    Blocks(Arc<Blocks>, Section),
    Legacy(Arc<Vec<u8>>),
    Loose(Arc<Mutex<File>>),
}

/// A seekable plaintext view of one asset. Clones have independent positions.
/// V2 readers retain one authenticated block each, including when restricted to a byte range.
#[derive(Clone)]
pub struct AssetReader {
    source: Source,
    start: u64,
    length: u64,
    position: u64,
    cached: Option<Arc<Block>>,
}

impl AssetReader {
    fn new(source: Source, start: u64, length: u64) -> Self {
        Self {
            source,
            start,
            length,
            position: 0,
            cached: None,
        }
    }
    pub(super) fn blocks(store: Arc<Blocks>, section: Section, start: u64, length: u64) -> Self {
        Self::new(Source::Blocks(store, section), start, length)
    }
    pub(super) fn legacy(bytes: Vec<u8>) -> Self {
        let length = bytes.len() as u64;
        Self::new(Source::Legacy(bytes.into()), 0, length)
    }
    pub(crate) fn loose(file: File) -> io::Result<Self> {
        let length = file.metadata()?.len();
        Ok(Self::new(
            Source::Loose(Arc::new(Mutex::new(file))),
            0,
            length,
        ))
    }
    pub fn len(&self) -> u64 {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Restrict the view to a half-open range within this asset and rewind it.
    pub fn range(mut self, range: Range<u64>) -> io::Result<Self> {
        if range.start > range.end || range.end > self.length {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Range outside asset",
            ));
        }
        self.start += range.start;
        self.length = range.end - range.start;
        self.position = 0;
        Ok(self)
    }
}

impl Read for AssetReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self
            .length
            .saturating_sub(self.position)
            .min(output.len() as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        let absolute = self.start + self.position;
        let copied = match &self.source {
            Source::Blocks(store, section) => {
                let block_size = u64::from(store.footer.chunk_size);
                let block = absolute / block_size;
                if self.cached.as_ref().map(|b| b.number) != Some(block) {
                    self.cached = Some(store.read_block(*section, block)?);
                }
                let bytes = &self.cached.as_ref().unwrap().bytes;
                let offset = (absolute % block_size) as usize;
                let n = count.min(bytes.len() - offset);
                output[..n].copy_from_slice(&bytes[offset..offset + n]);
                n
            }
            Source::Legacy(bytes) => {
                let start = absolute as usize;
                output[..count].copy_from_slice(&bytes[start..start + count]);
                count
            }
            Source::Loose(file) => {
                let mut file = file
                    .lock()
                    .map_err(|_| io::Error::other("Asset reader unavailable"))?;
                file.seek(SeekFrom::Start(absolute))?;
                file.read(&mut output[..count])?
            }
        };
        self.position += copied as u64;
        Ok(copied)
    }
}
impl Seek for AssetReader {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.position = match position {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::Current(n) => self.position.checked_add_signed(n),
            SeekFrom::End(n) => self.length.checked_add_signed(n),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid seek offset"))?;
        Ok(self.position)
    }
}
