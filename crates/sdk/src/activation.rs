//! Deliver Explorer files and application links to one process per profile and Windows session.
use crate::Result;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::{HANDLE, WAIT_TIMEOUT},
        Storage::FileSystem::{
            FILE_NOTIFY_CHANGE_FILE_NAME, FindCloseChangeNotification,
            FindFirstChangeNotificationW, FindNextChangeNotification,
        },
        System::{RemoteDesktop::ProcessIdToSessionId, Threading::WaitForSingleObject},
    },
    core::HSTRING,
};

struct DirectoryWatch(HANDLE);
impl Drop for DirectoryWatch {
    fn drop(&mut self) {
        unsafe {
            let _ = FindCloseChangeNotification(self.0);
        }
    }
}

const MAX_REQUEST: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Target {
    File(PathBuf),
    Url(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Activation {
    pub targets: Vec<Target>,
}

impl Activation {
    /// Resolve relative paths in the launching process. Only declared file types and schemes enter the queue.
    pub fn from_arguments(
        args: impl IntoIterator<Item = OsString>,
        file_types: &[&str],
        protocols: &[&str],
    ) -> Result<Self> {
        let mut targets = Vec::new();
        for arg in args {
            let text = arg
                .to_str()
                .ok_or("The activation argument is not Unicode")?;
            let scheme = text.split_once(':').map(|(name, _)| name);
            let target = if scheme
                .is_some_and(|name| protocols.iter().any(|p| p.eq_ignore_ascii_case(name)))
            {
                if text.len() > 8192 {
                    return Err("Link exceeds 8192 bytes".into());
                }
                if text.chars().any(char::is_control) {
                    return Err("Invalid control character in link".into());
                }
                Target::Url(text.to_owned())
            } else {
                if scheme.is_some_and(|name| {
                    !(name.len() == 1 && name.as_bytes()[0].is_ascii_alphabetic())
                }) {
                    return Err("Unregistered link scheme".into());
                }
                let path = PathBuf::from(arg);
                let ext = path
                    .extension()
                    .and_then(|v| v.to_str())
                    .ok_or("Expected a registered file extension")?;
                if !file_types.iter().any(|v| {
                    v.strip_prefix('.')
                        .is_some_and(|v| v.eq_ignore_ascii_case(ext))
                }) {
                    return Err(format!("Unregistered file extension: {ext}"));
                }
                Target::File(std::path::absolute(&path).map_err(|e| e.to_string())?)
            };
            targets.push(target);
        }
        Ok(Self { targets })
    }
}

/// Keep this guard alive in the primary process. The OS releases its file lock on exit or crash.
pub struct SingleInstance {
    _lease: File,
    directory: PathBuf,
    watch: Option<DirectoryWatch>,
    rescan: bool,
}

impl SingleInstance {
    /// Returns None after the running instance accepts the request. An empty activation raises its window.
    pub fn claim(profile: &Path, identity: &str, activation: &Activation) -> Result<Option<Self>> {
        if identity.is_empty()
            || identity.len() > 100
            || !identity
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
        {
            return Err("Invalid single-instance application identity".into());
        }
        let mut session = 0;
        unsafe { ProcessIdToSessionId(std::process::id(), &mut session) }
            .map_err(|e| e.to_string())?;
        let directory = profile
            .join("activations")
            .join(format!("app-{identity}-{session}"));
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("instance.lock"))
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(activation).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_REQUEST {
            return Err("Launch arguments exceed 64 KiB".into());
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut temp = tempfile::Builder::new()
            .prefix(&format!("{stamp:032}-"))
            .tempfile_in(&directory)
            .map_err(|e| e.to_string())?;
        temp.write_all(&bytes)
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        let request = temp.path().with_extension("ready");
        drop(
            temp.persist_noclobber(&request)
                .map_err(|e| e.to_string())?,
        );

        // If the primary exits while a launch is in flight, a waiting process takes over its queue.
        let deadline = Instant::now() + Duration::from_secs(45);
        while request.try_exists().map_err(|e| e.to_string())? {
            match lease.try_lock() {
                Ok(()) => {
                    return if request.try_exists().map_err(|e| e.to_string())? {
                        let watch = unsafe {
                            FindFirstChangeNotificationW(
                                &HSTRING::from(directory.as_os_str()),
                                false,
                                FILE_NOTIFY_CHANGE_FILE_NAME,
                            )
                        }
                        .ok()
                        .map(DirectoryWatch);
                        Ok(Some(Self {
                            watch,
                            rescan: true,
                            _lease: lease,
                            directory,
                        }))
                    } else {
                        Ok(None)
                    };
                }
                Err(TryLockError::Error(error)) => return Err(error.to_string()),
                Err(TryLockError::WouldBlock) => {}
            }
            if Instant::now() >= deadline {
                return Err("The running app has not acknowledged this launch. Its request is still queued.".into());
            }
            thread::sleep(Duration::from_millis(40));
        }
        Ok(None)
    }

    /// Acknowledge after the handler succeeds. A crash can replay its last unacknowledged request.
    pub fn dispatch(&mut self, mut receive: impl FnMut(Activation) -> Result<()>) -> Result<()> {
        // A quiet app checks an already-open kernel handle without enumerating its profile.
        if let Some(watch) = &self.watch {
            let signaled = unsafe { WaitForSingleObject(watch.0, 0) } != WAIT_TIMEOUT;
            if !self.rescan && !signaled {
                return Ok(());
            }
            if signaled && unsafe { FindNextChangeNotification(watch.0) }.is_err() {
                self.watch = None;
            }
        }
        let mut ready = Vec::new();
        for entry in fs::read_dir(&self.directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_some_and(|ext| ext == "ready") {
                ready.push(path);
            }
        }
        ready.sort();
        self.rescan = ready.len() > 32;
        for path in ready.into_iter().take(32) {
            let mut bytes = Vec::new();
            File::open(&path)
                .and_then(|f| f.take(MAX_REQUEST as u64 + 1).read_to_end(&mut bytes))
                .map_err(|e| e.to_string())?;
            if let Some(activation) = (bytes.len() <= MAX_REQUEST)
                .then(|| serde_json::from_slice(&bytes).ok())
                .flatten()
            {
                receive(activation)?;
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            } else {
                fs::rename(&path, path.with_extension("invalid")).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
