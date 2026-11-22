//! Platform-independent protocol and stable, C-compatible SDK types.
use serde::Deserialize;
use serde_json::{Value, json};
pub const ABI_VERSION: u32 = 1;
pub const MAX_MESSAGE: usize = 256 * 1024;
pub const APP_ORIGIN: &str = "https://rustybox.local";
pub const OK: i32 = 0;
pub const CLOSED: i32 = 1;
pub const EMPTY: i32 = 2;
pub const PENDING: i32 = 3;
pub const INVALID_ARGUMENT: i32 = -1;
pub const INVALID_HANDLE: i32 = -2;
pub const PLATFORM_ERROR: i32 = -3;
pub const ABI_MISMATCH: i32 = -4;
pub const BUFFER_TOO_SMALL: i32 = -5;
pub const BUSY: i32 = -6;
pub const DEBUG: u32 = 1;
pub const HIDDEN: u32 = 2;
pub const SELF_TEST: u32 = 4;
pub const COMPOSED: u32 = 8;
pub const RESERVED_ORIGIN: u32 = 16;

/// All strings are borrowed UTF-8 byte spans, copied during rb_create.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Config {
    pub struct_size: u32,
    pub abi_version: u32,
    pub parent_window: usize,
    pub width: u32,
    pub height: u32,
    pub flags: u32,
    pub reserved: u32,
    pub title: *const u8,
    pub title_len: usize,
    pub assets: *const u8,
    pub assets_len: usize,
    pub data_dir: *const u8,
    pub data_dir_len: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            struct_size: std::mem::size_of::<Self>() as u32,
            abi_version: ABI_VERSION,
            parent_window: 0,
            width: 1440,
            height: 940,
            flags: 0,
            reserved: 0,
            title: std::ptr::null(),
            title_len: 0,
            assets: std::ptr::null(),
            assets_len: 0,
            data_dir: std::ptr::null(),
            data_dir_len: 0,
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: u32,
    pub id: u64,
    #[serde(default)]
    pub session: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}
pub fn parse_request(raw: &str) -> Result<Request, String> {
    if raw.len() > MAX_MESSAGE {
        return Err("Message exceeds 256 KiB".into());
    }
    let request: Request = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if request.v != ABI_VERSION {
        return Err("Unsupported bridge version".into());
    }
    if request.id == 0 || request.id > 9_007_199_254_740_991 {
        return Err("Request ID must be a positive JavaScript-safe integer".into());
    }
    if request.session.len() > 80
        || !request
            .session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Invalid document session".into());
    }
    if request.method.is_empty()
        || request.method.len() > 80
        || !request
            .method
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_')
    {
        return Err("Invalid command name".into());
    }
    Ok(request)
}
/// The selected HTTPS origin owns this view's Web Storage, cookies and IndexedDB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetOrigin {
    LegacyLocal,
    Reserved,
}
impl AssetOrigin {
    pub fn base(self) -> &'static str {
        match self {
            Self::LegacyLocal => APP_ORIGIN,
            Self::Reserved => "https://rustybox.example",
        }
    }
    pub fn host(self) -> &'static str {
        self.base().trim_start_matches("https://")
    }
    pub fn trusts(self, url: &str) -> bool {
        url.strip_prefix(self.base())
            .is_some_and(|path| path.is_empty() || path.starts_with('/'))
    }
}
pub fn trusted_url(url: &str) -> bool {
    AssetOrigin::LegacyLocal.trusts(url)
}
pub fn response(id: u64, session: &str, result: Result<Value, String>) -> Value {
    match result {
        Ok(value) => json!({"v": 1, "id": id, "session": session, "ok": true, "result": value}),
        Err(error) => {
            json!({"v": 1, "id": id, "session": session, "ok": false, "error": {"code": "COMMAND_FAILED", "message": error}})
        }
    }
}
pub fn storage_key(value: &Value) -> Result<&str, String> {
    let key = value
        .get("key")
        .and_then(Value::as_str)
        .ok_or("Missing storage key")?;
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("Invalid storage key".into());
    }
    Ok(key)
}
