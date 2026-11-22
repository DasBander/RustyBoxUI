//! RustyBoxUI's versioned C ABI. All view operations run on the creating STA thread.
#![cfg(windows)]
mod composition;
mod graphics;
mod platform;
mod storage;
use rustybox_core::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
};
use std::{ffi::c_void, time::Duration};
use windows::Win32::UI::WindowsAndMessaging::*;
thread_local! {
    static ERROR: RefCell<String> = const { RefCell::new(String::new()) };
    static ENTERED: Cell<bool> = const { Cell::new(false) };
}
fn fail(code: i32, message: impl Into<String>) -> i32 {
    ERROR.with(|e| *e.borrow_mut() = message.into());
    code
}
// WebView2 may dispatch a window message while an ABI call is still on the stack.
// Reject reentry and turn unwinding Rust failures into error codes for the C caller.
fn guard(f: impl FnOnce() -> Result<i32, String>) -> i32 {
    if ENTERED.with(|entered| entered.replace(true)) {
        return fail(BUSY, "Reentrant SDK calls are not supported");
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        platform::collect();
        f()
    }));
    ENTERED.with(|e| e.set(false));
    match result {
        Ok(Ok(code)) => code,
        Ok(Err(e)) => fail(PLATFORM_ERROR, e),
        Err(_) => fail(PLATFORM_ERROR, "Rust panic contained at the ABI boundary"),
    }
}
unsafe fn utf8(ptr: *const u8, len: usize) -> Result<String, String> {
    if len == 0 {
        return Ok(String::new());
    }
    if ptr.is_null() || len > MAX_MESSAGE {
        return Err("Invalid UTF-8 span".into());
    }
    unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(ptr, len))
            .map(str::to_owned)
            .map_err(|e| e.to_string())
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn rb_abi_version() -> u32 {
    ABI_VERSION
}
/// # Safety
/// Config and out must be valid aligned pointers. Every string must reference len readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_create(config: *const Config, out: *mut u64) -> i32 {
    unsafe { create(config, out, 30_000, true) }
}
/// Start view creation without pumping messages. The returned ticket becomes the view handle on success.
/// # Safety
/// Same pointer requirements as rb_create.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_create_begin(
    config: *const Config,
    timeout_ms: u32,
    out: *mut u64,
) -> i32 {
    unsafe { create(config, out, timeout_ms, false) }
}
unsafe fn create(config: *const Config, out: *mut u64, timeout_ms: u32, synchronous: bool) -> i32 {
    if config.is_null() || out.is_null() {
        return fail(INVALID_ARGUMENT, "Null config or output");
    }
    unsafe {
        *out = 0;
    }
    if unsafe { config.cast::<u32>().read() } != std::mem::size_of::<Config>() as u32 {
        return fail(ABI_MISMATCH, "Config size mismatch");
    }
    let config = unsafe { *config };
    if config.abi_version != ABI_VERSION {
        return fail(ABI_MISMATCH, "ABI version mismatch");
    }
    if config.width < 320
        || config.height < 240
        || config.width > 16384
        || config.height > 16384
        || !(1..=300_000).contains(&timeout_ms)
        || config.flags & !(DEBUG | HIDDEN | SELF_TEST | COMPOSED | RESERVED_ORIGIN) != 0
        || config.reserved != 0
    {
        return fail(
            INVALID_ARGUMENT,
            "Invalid dimensions, flags, or reserved field",
        );
    }
    guard(|| {
        let title = unsafe { utf8(config.title, config.title_len) }?;
        let assets = unsafe { utf8(config.assets, config.assets_len) }?;
        let data = unsafe { utf8(config.data_dir, config.data_dir_len) }?;
        if assets.is_empty() {
            return Err("An asset directory containing index.html is required".into());
        }
        let data = if data.is_empty() {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA missing")?)
                .join("RustyBoxUI")
                .join("default")
        } else {
            PathBuf::from(data)
        };
        let id = platform::begin(
            config,
            if title.is_empty() {
                "RustyBoxUI".into()
            } else {
                title
            },
            PathBuf::from(assets),
            data,
            Duration::from_millis(timeout_ms.into()),
        )?;
        if synchronous {
            loop {
                if !platform::pump() {
                    platform::cancel(id);
                    return Err("Quit during view creation".into());
                }
                if platform::finish(id)? != PENDING {
                    break;
                }
                platform::wait(8);
            }
        }
        unsafe {
            *out = id;
        }
        Ok(OK)
    })
}
/// Call between turns of the host message loop. A terminal status consumes the startup ticket.
#[unsafe(no_mangle)]
pub extern "C" fn rb_create_status(id: u64) -> i32 {
    guard(|| platform::finish(id))
}
#[unsafe(no_mangle)]
pub extern "C" fn rb_create_cancel(id: u64) -> i32 {
    guard(|| {
        Ok(if platform::cancel(id) {
            OK
        } else {
            INVALID_HANDLE
        })
    })
}
/// Poll after a cancellation until OK to reclaim delayed WebView2 callbacks. Pump the STA between calls.
#[unsafe(no_mangle)]
pub extern "C" fn rb_collect_startups() -> i32 {
    guard(|| Ok(if platform::collect() { OK } else { PENDING }))
}
#[unsafe(no_mangle)]
pub extern "C" fn rb_wait(id: u64, maximum_ms: u32) -> i32 {
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(INVALID_HANDLE);
        };
        if !session.closed.get() {
            platform::wait(session.wait_ms(maximum_ms));
        }
        Ok(OK)
    })
}
/// # Safety
/// out receives a retained ID3D11Device pointer. Release it on the view thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_graphics_device(id: u64, out: *mut *mut c_void) -> i32 {
    if out.is_null() {
        return INVALID_ARGUMENT;
    }
    unsafe {
        *out = std::ptr::null_mut();
    }
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(INVALID_HANDLE);
        };
        session
            .graphics
            .borrow()
            .as_ref()
            .ok_or("Attach the graphics viewport first")?
            .device(out)?;
        Ok(OK)
    })
}
/// # Safety
/// texture is a valid ID3D11Texture2D or null. The runtime holds an independent reference after success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_graphics_texture(id: u64, texture: *mut c_void) -> i32 {
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(INVALID_HANDLE);
        };
        session
            .graphics
            .borrow_mut()
            .as_mut()
            .ok_or("Attach the graphics viewport first")?
            .texture(texture)?;
        Ok(OK)
    })
}
/// Discard and recreate an attached graphics device. The host resubmits its texture on graphics_restored.
#[unsafe(no_mangle)]
pub extern "C" fn rb_graphics_reset(id: u64) -> i32 {
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(INVALID_HANDLE);
        };
        session.reset_graphics();
        Ok(OK)
    })
}
/// Pump messages for standalone hosts. Embedded hosts use their own loop plus rb_tick.
#[unsafe(no_mangle)]
pub extern "C" fn rb_poll(id: u64) -> i32 {
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(fail(INVALID_HANDLE, "Unknown handle or wrong thread"));
        };
        if !platform::pump() {
            session.closed.set(true);
        }
        if session.closed.get() {
            Ok(CLOSED)
        } else {
            session.tick();
            Ok(OK)
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rb_tick(id: u64) -> i32 {
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(fail(INVALID_HANDLE, "Unknown handle or wrong thread"));
        };
        if session.closed.get() {
            Ok(CLOSED)
        } else {
            session.tick();
            Ok(OK)
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rb_resize(id: u64, width: u32, height: u32) -> i32 {
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return fail(INVALID_ARGUMENT, "Invalid dimensions");
    }
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(fail(INVALID_HANDLE, "Unknown handle or wrong thread"));
        };
        unsafe {
            SetWindowPos(
                session.hwnd,
                None,
                0,
                0,
                width as i32,
                height as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(OK)
    })
}
/// # Safety
/// json must reference length readable UTF-8 bytes; the message must be a JSON object.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_post_json(id: u64, json: *const u8, length: usize) -> i32 {
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(fail(INVALID_HANDLE, "Unknown handle or wrong thread"));
        };
        let raw = unsafe { utf8(json, length) }?;
        let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if !value.is_object() {
            return Ok(fail(INVALID_ARGUMENT, "Message must be an object"));
        }
        session.post(value)?;
        Ok(OK)
    })
}
/// Read one queued UTF-8 JSON event. Call with a null buffer and zero capacity to
/// obtain its byte length. The event remains queued until a copy succeeds.
/// # Safety
/// written must be writable; buffer must reference capacity writable bytes, or be null with capacity zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_next_event(
    id: u64,
    buffer: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> i32 {
    if written.is_null() || (buffer.is_null() && capacity != 0) {
        return fail(INVALID_ARGUMENT, "Invalid output buffer");
    }
    unsafe {
        *written = 0;
    }
    guard(|| {
        let Some(session) = platform::get(id) else {
            return Ok(fail(INVALID_HANDLE, "Unknown handle or wrong thread"));
        };
        let mut queue = session.events.borrow_mut();
        let Some(event) = queue.front() else {
            return Ok(EMPTY);
        };
        unsafe {
            *written = event.len();
        }
        if capacity < event.len() {
            return Ok(BUFFER_TOO_SMALL);
        }
        unsafe {
            std::ptr::copy_nonoverlapping(event.as_ptr(), buffer, event.len());
        }
        queue.pop_front();
        Ok(OK)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn rb_native_handle(id: u64) -> usize {
    platform::get(id).map_or(0, |s| s.hwnd.0 as usize)
}
#[unsafe(no_mangle)]
pub extern "C" fn rb_destroy(id: u64) -> i32 {
    guard(|| {
        if platform::destroy(id) {
            Ok(OK)
        } else {
            Ok(fail(INVALID_HANDLE, "Unknown handle or wrong thread"))
        }
    })
}
/// Returns required byte count excluding NUL. Copies a truncated NUL-terminated string if capacity > 0.
/// # Safety
/// buffer must reference capacity writable bytes, or be null with capacity zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rb_last_error(buffer: *mut u8, capacity: usize) -> usize {
    ERROR.with(|error| {
        let error = error.borrow();
        let bytes = error.as_bytes();
        if !buffer.is_null() && capacity > 0 {
            let len = bytes.len().min(capacity - 1);
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, len);
                *buffer.add(len) = 0;
            }
        }
        bytes.len()
    })
}
