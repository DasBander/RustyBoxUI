use rustybox_core::{MAX_MESSAGE, Request, storage_key};
use serde_json::{Map, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, Weak,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_NULL},
};

pub type Reply = (u64, String, Result<Value, String>);
struct Job {
    request: Request,
    reply: SyncSender<Reply>,
    wake: usize,
}
struct Profile {
    tx: Option<SyncSender<Job>>,
    worker: Option<JoinHandle<()>>,
    _lock: std::fs::File,
}
static PROFILES: LazyLock<Mutex<HashMap<PathBuf, Weak<Profile>>>> = LazyLock::new(Default::default);
pub struct Storage {
    profile: Arc<Profile>,
    sender: SyncSender<Reply>,
    replies: Receiver<Reply>,
    pending: AtomicUsize,
    wake: usize,
}

impl Profile {
    fn open(directory: &Path) -> Result<Self, String> {
        use std::os::windows::fs::OpenOptionsExt;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .share_mode(0)
            .open(directory.join("settings.lock"))
            .map_err(|e| format!("Another process owns the profile or it is inaccessible: {e}"))?;
        let path = directory.join("settings.json");
        let mut values = match std::fs::read(&path) {
            Ok(bytes) if bytes.len() <= MAX_MESSAGE => {
                serde_json::from_slice(&bytes).map_err(|e| format!("Invalid settings file: {e}"))?
            }
            Ok(_) => return Err("Settings file exceeds 256 KiB".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
            Err(e) => return Err(e.to_string()),
        };
        let (tx, rx) = mpsc::sync_channel::<Job>(128);
        let worker = thread::Builder::new()
            .name("rustybox-storage".into())
            .spawn(move || {
                while let Ok(Job {
                    request,
                    reply,
                    wake,
                }) = rx.recv()
                {
                    let result = access(&path, &mut values, &request);
                    // Each view reserves a reply slot before enqueueing. A closed receiver is harmless.
                    if reply
                        .try_send((request.id, request.session, result))
                        .is_ok()
                        && wake != 0
                    {
                        unsafe {
                            let _ = PostMessageW(
                                Some(HWND(wake as *mut _)),
                                WM_NULL,
                                WPARAM(0),
                                LPARAM(0),
                            );
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            tx: Some(tx),
            worker: Some(worker),
            _lock: lock,
        })
    }
}
fn access(
    path: &Path,
    values: &mut Map<String, Value>,
    request: &Request,
) -> Result<Value, String> {
    let key = storage_key(&request.params)?.to_owned();
    if request.method == "storage.get" {
        return Ok(values.get(&key).cloned().unwrap_or(Value::Null));
    }
    let value = request.params.get("value").ok_or("Missing value")?.clone();
    let mut next = values.clone();
    next.insert(key, value.clone());
    let bytes = serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_MESSAGE {
        return Err("Settings quota of 256 KiB exceeded".into());
    }
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
    }
    std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
    *values = next;
    Ok(value)
}
impl Storage {
    pub fn open(directory: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let key = directory.canonicalize().map_err(|e| e.to_string())?;
        let mut profiles = PROFILES
            .lock()
            .map_err(|_| "Profile registry unavailable")?;
        profiles.retain(|_, p| p.strong_count() != 0);
        let profile = if let Some(existing) = profiles.get(&key).and_then(Weak::upgrade) {
            existing
        } else {
            let profile = Arc::new(Profile::open(&key)?);
            profiles.insert(key, Arc::downgrade(&profile));
            profile
        };
        let (sender, replies) = mpsc::sync_channel(128);
        Ok(Self {
            profile,
            sender,
            replies,
            pending: AtomicUsize::new(0),
            wake: 0,
        })
    }
    pub fn set_wakeup(&mut self, hwnd: HWND) {
        self.wake = hwnd.0 as usize;
    }
    pub fn submit(&self, request: Request) -> Result<(), String> {
        if self.pending.load(Ordering::Relaxed) >= 128 {
            return Err("View storage queue full".into());
        }
        self.profile
            .tx
            .as_ref()
            .ok_or("Storage closed")?
            .try_send(Job {
                request,
                reply: self.sender.clone(),
                wake: self.wake,
            })
            .map_err(|_| "Profile storage queue full")?;
        self.pending.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    pub fn drain(&self) -> impl Iterator<Item = Reply> + '_ {
        self.replies.try_iter().inspect(|_| {
            self.pending.fetch_sub(1, Ordering::Relaxed);
        })
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
