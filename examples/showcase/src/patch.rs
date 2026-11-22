use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

const CHUNK: usize = 32 * 1024;
type Payloads = Vec<(&'static str, Vec<u8>)>;
fn payloads(assets: &rustybox_assets::Assets) -> Result<Payloads, String> {
    let read = |path: &str| {
        assets
            .read(path)?
            .ok_or_else(|| format!("Missing patch payload {path}"))
    };
    Ok(vec![
        ("relay-keyart.webp", read("art/signal.webp")?),
        ("transmission.wav", read("_native/transmission.wav")?),
        ("relay-stations.json", read("_native/relay-stations.json")?),
    ])
}
#[derive(Clone, Serialize)]
pub struct FileStatus {
    name: &'static str,
    bytes: usize,
    verified: usize,
}
#[derive(Clone, Serialize)]
pub struct Status {
    #[serde(skip)]
    applied: u64,
    state: &'static str,
    files: Vec<FileStatus>,
    total: usize,
    verified: usize,
    error: Option<String>,
}
impl Status {
    fn new(files: &Payloads) -> Self {
        Self {
            applied: 0,
            state: "checking",
            total: files.iter().map(|f| f.1.len()).sum(),
            verified: 0,
            files: files
                .iter()
                .map(|(name, bytes)| FileStatus {
                    name,
                    bytes: bytes.len(),
                    verified: 0,
                })
                .collect(),
            error: None,
        }
    }
    fn recount(&mut self) {
        self.verified = self.files.iter().map(|f| f.verified).sum();
    }
}

// Only full, matching chunks count as resumable. The last chunk may be shorter.
fn valid_prefix(path: &Path, source: &[u8]) -> io::Result<usize> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut block = vec![0; CHUNK];
    let mut valid = 0;
    for expected in source.chunks(CHUNK) {
        let dest = &mut block[..expected.len()];
        match file.read_exact(dest) {
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            other => other?,
        }
        if Sha256::digest(&*dest) != Sha256::digest(expected) {
            break;
        }
        valid += expected.len();
    }
    Ok(valid)
}
fn part(root: &Path, name: &str) -> PathBuf {
    root.join(format!("{name}.part"))
}

fn scan(root: &Path, status: &mut Status, files: &Payloads) -> io::Result<()> {
    fs::create_dir_all(root)?;
    for (index, (name, source)) in files.iter().enumerate() {
        let installed = root.join(name);
        let size = source.len();
        let valid = if valid_prefix(&installed, source)? == size
            && fs::metadata(&installed)?.len() == size as u64
        {
            size
        } else {
            let staging = part(root, name);
            let prefix = valid_prefix(&staging, source)?;
            if staging.try_exists()? {
                OpenOptions::new()
                    .write(true)
                    .open(&staging)?
                    .set_len(prefix as u64)?;
                if prefix == size {
                    fs::rename(staging, &installed)?;
                }
            }
            prefix
        };
        status.files[index].verified = valid;
    }
    status.recount();
    status.error = None;
    status.state = if status.verified == status.total {
        "complete"
    } else if status.verified > 0 {
        "paused"
    } else {
        "ready"
    };
    Ok(())
}
fn install_chunk(root: &Path, status: &mut Status, files: &Payloads) -> io::Result<()> {
    let Some(index) = status.files.iter().position(|f| f.verified < f.bytes) else {
        return Ok(());
    };
    let (name, bytes) = &files[index];
    let start = status.files[index].verified;
    let end = (start + CHUNK).min(bytes.len());
    let path = part(root, name);
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)?;
    f.seek(SeekFrom::Start(start as u64))?;
    f.write_all(&bytes[start..end])?;
    f.sync_data()?;
    let mut check = vec![0; end - start];
    f.seek(SeekFrom::Start(start as u64))?;
    f.read_exact(&mut check)?;
    if Sha256::digest(&check) != Sha256::digest(&bytes[start..end]) {
        return Err(io::Error::other("Chunk integrity check failed"));
    }
    drop(f);
    if end == bytes.len() {
        if valid_prefix(&path, bytes)? != end || fs::metadata(&path)?.len() != end as u64 {
            return Err(io::Error::other("Final file integrity check failed"));
        }
        fs::rename(&path, root.join(name))?;
    }
    status.files[index].verified = end;
    status.recount();
    if status.verified == status.total {
        status.state = "complete";
    }
    Ok(())
}

pub enum Action {
    Start,
    Pause,
    Verify,
}
pub struct Worker {
    status: Arc<Mutex<Status>>,
    tx: Option<mpsc::Sender<(u64, Action)>>,
    requested: u64,
    thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn new(root: PathBuf, assets: &rustybox_assets::Assets) -> Result<Self, String> {
        let files = payloads(assets)?;
        let shared = Arc::new(Mutex::new(Status::new(&files)));
        let (tx, rx) = mpsc::channel();
        let publish = shared.clone();
        let thread = thread::spawn(move || {
            let mut status = Status::new(&files);
            let mut result = scan(&root, &mut status, &files);
            loop {
                if let Err(e) = result {
                    status.state = "error";
                    status.error = Some(e.to_string());
                }
                *publish.lock().unwrap() = status.clone();
                let action = if status.state == "running" {
                    match rx.recv_timeout(Duration::from_millis(85)) {
                        Ok(a) => Some(a),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(_) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(a) => Some(a),
                        Err(_) => break,
                    }
                };
                let action = action.map(|(id, action)| {
                    status.applied = id;
                    action
                });
                result = match action {
                    Some(Action::Verify) => scan(&root, &mut status, &files),
                    Some(Action::Start) => scan(&root, &mut status, &files).map(|_| {
                        if status.verified < status.total {
                            status.state = "running";
                        }
                    }),
                    Some(Action::Pause) => {
                        if status.state == "running" {
                            status.state = "paused";
                        }
                        Ok(())
                    }
                    None => install_chunk(&root, &mut status, &files),
                };
            }
        });
        Ok(Self {
            status: shared,
            requested: 0,
            tx: Some(tx),
            thread: Some(thread),
        })
    }
    pub fn read(&self) -> Status {
        let mut status = self.status.lock().unwrap().clone();
        if status.applied < self.requested {
            status.state = "checking";
        }
        status
    }
    pub fn send(&mut self, action: Action) -> Result<(), String> {
        self.requested += 1;
        self.tx
            .as_ref()
            .unwrap()
            .send((self.requested, action))
            .map_err(|e| e.to_string())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
