//! WebView2 reads response bodies asynchronously, sometimes from a different COM apartment.
use rustybox_assets::AssetReader;
use std::{
    ffi::c_void,
    io::{Read, Seek, SeekFrom},
    ptr,
    sync::{Mutex, MutexGuard, OnceLock},
};
use windows::{
    Win32::{
        Foundation::*,
        System::{Com::*, LibraryLoader::*},
    },
    core::{HRESULT, PCWSTR, Ref, Result, implement},
};

#[implement(IStream)]
struct ResourceStream {
    reader: Mutex<AssetReader>,
}

pub(super) fn open(reader: AssetReader) -> Result<IStream> {
    // WebView2 may still own the agile IStream after the host destroys its window and unloads its DLL.
    // Keep the COM vtable mapped until process exit, including during its final IUnknown::Release.
    static PIN: OnceLock<HRESULT> = OnceLock::new();
    PIN.get_or_init(|| unsafe {
        let mut module = HMODULE::default();
        match GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            PCWSTR(open as *const () as _),
            &mut module,
        ) {
            Ok(()) => S_OK,
            Err(e) => e.code(),
        }
    })
    .ok()?;
    Ok(ResourceStream {
        reader: Mutex::new(reader),
    }
    .into())
}

impl ResourceStream_Impl {
    fn lock_reader(&self) -> Result<MutexGuard<'_, AssetReader>> {
        self.reader.lock().map_err(|_| STG_E_READFAULT.into())
    }
}

impl ISequentialStream_Impl for ResourceStream_Impl {
    fn Read(&self, output: *mut c_void, count: u32, written: *mut u32) -> HRESULT {
        if !written.is_null() {
            unsafe { written.write(0) };
        }
        if count == 0 {
            return S_OK;
        }
        if output.is_null() {
            return E_POINTER;
        }
        let Ok(mut reader) = self.lock_reader() else {
            return STG_E_READFAULT;
        };
        // A COM caller can pass uninitialized memory. Read into initialized Rust storage before copying.
        let mut scratch = [0u8; 16 * 1024];
        let mut copied = 0;
        while copied < count {
            let take = ((count - copied) as usize).min(scratch.len());
            match reader.read(&mut scratch[..take]) {
                Ok(0) => return S_FALSE,
                Ok(n) => unsafe {
                    ptr::copy_nonoverlapping(
                        scratch.as_ptr(),
                        output.cast::<u8>().add(copied as usize),
                        n,
                    );
                    copied += n as u32;
                    if !written.is_null() {
                        written.write(copied);
                    }
                },
                Err(_) => return STG_E_READFAULT,
            }
        }
        S_OK
    }

    fn Write(&self, _: *const c_void, _: u32, written: *mut u32) -> HRESULT {
        if !written.is_null() {
            unsafe { written.write(0) };
        }
        STG_E_ACCESSDENIED
    }
}

impl IStream_Impl for ResourceStream_Impl {
    fn Seek(&self, offset: i64, origin: STREAM_SEEK, position: *mut u64) -> Result<()> {
        let from = match origin {
            STREAM_SEEK_SET if offset >= 0 => SeekFrom::Start(offset as u64),
            STREAM_SEEK_CUR => SeekFrom::Current(offset),
            STREAM_SEEK_END => SeekFrom::End(offset),
            _ => return Err(STG_E_INVALIDFUNCTION.into()),
        };
        let next = self
            .lock_reader()?
            .seek(from)
            .map_err(|_| STG_E_INVALIDFUNCTION)?;
        if !position.is_null() {
            unsafe { position.write(next) };
        }
        Ok(())
    }

    fn CopyTo(
        &self,
        destination: Ref<IStream>,
        count: u64,
        read: *mut u64,
        written: *mut u64,
    ) -> Result<()> {
        let mut consumed = 0;
        let mut sent = 0;
        let result = (|| {
            let destination = destination.ok()?;
            let mut buffer = [0u8; 16 * 1024];
            while consumed < count {
                let take = (count - consumed).min(buffer.len() as u64) as usize;
                let n = self
                    .lock_reader()?
                    .read(&mut buffer[..take])
                    .map_err(|_| STG_E_READFAULT)?;
                if n == 0 {
                    break;
                }
                consumed += n as u64;
                // Drop the cursor lock before entering another COM object.
                let mut received = 0;
                let status = unsafe {
                    destination.Write(buffer.as_ptr().cast(), n as u32, Some(&mut received))
                };
                sent += u64::from(received);
                status.ok()?;
                if received as usize != n {
                    return Err(STG_E_MEDIUMFULL.into());
                }
            }
            Ok(())
        })();
        unsafe {
            if !read.is_null() {
                read.write(consumed);
            }
            if !written.is_null() {
                written.write(sent);
            }
        }
        result
    }

    fn Stat(&self, stat: *mut STATSTG, _: &STATFLAG) -> Result<()> {
        if stat.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe {
            stat.write(STATSTG {
                r#type: STGTY_STREAM.0 as u32,
                cbSize: self.lock_reader()?.len(),
                grfMode: STGM_READ,
                ..Default::default()
            });
        }
        Ok(())
    }
    fn Clone(&self) -> Result<IStream> {
        open(self.lock_reader()?.clone())
    }
    fn SetSize(&self, _: u64) -> Result<()> {
        Err(STG_E_ACCESSDENIED.into())
    }
    fn Commit(&self, _: &STGC) -> Result<()> {
        Ok(())
    }
    fn Revert(&self) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }
    fn LockRegion(&self, _: u64, _: u64, _: &LOCKTYPE) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }
    fn UnlockRegion(&self, _: u64, _: u64, _: u32) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }
}
