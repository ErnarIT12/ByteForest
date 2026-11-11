//! Raw volume handle: `CreateFileW` + `SetFilePointerEx` + `ReadFile`, closed on drop.
#![allow(unsafe_code)]

use std::fmt::Display;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;

use windows::Win32::Foundation::{GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_BEGIN, FILE_FLAG_NO_BUFFERING, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, SetFilePointerEx,
};
use windows::core::PCWSTR;

/// Largest single `ReadFile` call; bigger reads are split into several calls.
pub(crate) const MAX_READ: usize = 1 << 20;

/// Wraps `err` with the step that failed, keeping its kind: `"{step}: {err}"`.
pub(crate) fn context(step: impl Display, err: io::Error) -> io::Error {
    io::Error::new(err.kind(), format!("{step}: {err}"))
}

/// Converts a `windows` crate error into an `io::Error` with the plain Win32 error code, so
/// that `kind()` works (e.g. `PermissionDenied`) and the message names the code.
///
/// `io::Error::from(windows::core::Error)` keeps the whole HRESULT (`0x8007xxxx`) as the OS
/// error code, which `io::Error` cannot classify.
pub(crate) fn win32_error(err: windows::core::Error) -> io::Error {
    let hresult = err.code().0 as u32;
    if hresult & 0xFFFF_0000 == 0x8007_0000 {
        io::Error::from_raw_os_error((hresult & 0xFFFF) as i32)
    } else {
        io::Error::other(err)
    }
}

/// An open volume (or, in tests, an ordinary file) for raw sector reads.
///
/// `OwnedHandle` closes the handle on drop and is `Send + Sync`, so a reader thread can
/// borrow it. Reads move the shared file pointer, so only one thread may read at a time.
pub(crate) struct Volume {
    handle: OwnedHandle,
    path: String,
    unbuffered: bool,
}

impl Volume {
    /// Opens `\\.\X:` with `GENERIC_READ` and share read|write; `unbuffered` adds
    /// `FILE_FLAG_NO_BUFFERING`.
    pub fn open_drive(letter: char, unbuffered: bool) -> io::Result<Self> {
        Self::open_path(Path::new(&format!(r"\\.\{letter}:")), unbuffered)
    }

    /// Opens any path the same way (used by tests with a plain file as a fake volume).
    pub fn open_path(path: &Path, unbuffered: bool) -> io::Result<Self> {
        let display = path.display().to_string();
        let wide: Vec<u16> = display.encode_utf16().chain([0]).collect();
        let flags = if unbuffered {
            FILE_FLAG_NO_BUFFERING
        } else {
            FILE_FLAGS_AND_ATTRIBUTES(0)
        };
        let mode = if unbuffered { "unbuffered" } else { "buffered" };
        // SAFETY: `wide` is NUL-terminated and outlives the call; no security attributes or
        // template handle are passed.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                flags,
                None,
            )
        }
        .map_err(|e| context(format!("open {display} ({mode})"), win32_error(e)))?;
        // SAFETY: `CreateFileW` succeeded, so `handle` is a valid handle that nothing else
        // owns; `OwnedHandle` takes sole ownership and closes it exactly once.
        let handle = unsafe { OwnedHandle::from_raw_handle(handle.0) };
        Ok(Self {
            handle,
            path: display,
            unbuffered,
        })
    }

    /// The raw handle, for `DeviceIoControl`. Valid while `self` lives.
    pub fn raw(&self) -> HANDLE {
        HANDLE(self.handle.as_raw_handle())
    }

    /// Path the volume was opened with, for messages.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Reads exactly `buf.len()` bytes at byte `offset`, in calls of at most [`MAX_READ`]
    /// bytes, retrying after short reads. Errors name the offset and length that failed.
    pub fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let mut done = 0usize;
        while done < buf.len() {
            let pos = offset + done as u64;
            let len = (buf.len() - done).min(MAX_READ);
            let step = || {
                format!(
                    "read {len} bytes at volume offset {pos:#x} of {} ({})",
                    self.path,
                    if self.unbuffered {
                        "unbuffered"
                    } else {
                        "buffered"
                    }
                )
            };
            let distance = i64::try_from(pos).map_err(|_| {
                context(
                    step(),
                    io::Error::new(io::ErrorKind::InvalidInput, "offset too large"),
                )
            })?;
            // SAFETY: the handle is valid while `self` lives; no new-position out-pointer.
            unsafe { SetFilePointerEx(self.raw(), distance, None, FILE_BEGIN) }
                .map_err(|e| context(format!("seek before {}", step()), win32_error(e)))?;

            let mut read = 0u32;
            let target = &mut buf[done..done + len];
            // SAFETY: the handle is valid while `self` lives and was opened for synchronous
            // I/O; `target` is a writable buffer of `len` bytes (at most 1 MiB, fits `u32`)
            // and `read` is a valid out-pointer. No OVERLAPPED is passed.
            unsafe { ReadFile(self.raw(), Some(target), Some(&mut read), None) }
                .map_err(|e| context(step(), win32_error(e)))?;
            if read == 0 {
                return Err(context(
                    step(),
                    io::Error::new(io::ErrorKind::UnexpectedEof, "read returned 0 bytes"),
                ));
            }
            done += read as usize;
        }
        Ok(())
    }
}
