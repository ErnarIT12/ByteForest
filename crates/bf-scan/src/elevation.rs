//! Check whether the process runs with an elevated (administrator) token.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Returns `true` if the current process token is elevated; any failure counts as `false`.
pub fn is_elevated() -> bool {
    let mut token = HANDLE::default();
    // SAFETY: `GetCurrentProcess` returns a pseudo-handle that needs no cleanup, and `token`
    // is a valid out-pointer for the duration of the call.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }

    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    // SAFETY: `token` is a valid token handle opened above; the buffer pointer and length
    // describe `elevation`, and `returned` is a valid out-pointer.
    let queried = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut TOKEN_ELEVATION as *mut c_void),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    // SAFETY: `token` was opened by this function and is closed exactly once.
    let _ = unsafe { CloseHandle(token) };

    queried.is_ok() && elevation.TokenIsElevated != 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn is_elevated_does_not_panic() {
        let _ = super::is_elevated();
    }
}
