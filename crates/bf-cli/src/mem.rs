//! Process memory statistics.

#[cfg(windows)]
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
#[cfg(windows)]
use windows::Win32::System::Threading::GetCurrentProcess;

/// Peak working set of this process in bytes, or `None` if the query fails.
#[cfg(windows)]
pub fn peak_working_set() -> Option<u64> {
    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: `GetCurrentProcess` returns a pseudo-handle that needs no closing, and
    // `counters` is a valid, writable struct whose size is passed as `cb`.
    let result = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) };
    result.ok().map(|()| counters.PeakWorkingSetSize as u64)
}

#[cfg(target_os = "macos")]
pub fn peak_working_set() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: the pointer is writable; the struct is initialized on success.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    // Darwin reports ru_maxrss in bytes.
    u64::try_from(unsafe { usage.assume_init() }.ru_maxrss).ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn peak_working_set() -> Option<u64> {
    None
}
