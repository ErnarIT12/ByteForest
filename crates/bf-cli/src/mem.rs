//! Process memory statistics.

use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;

/// Peak working set of this process in bytes, or `None` if the query fails.
pub fn peak_working_set() -> Option<u64> {
    let mut counters = PROCESS_MEMORY_COUNTERS::default();
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: `GetCurrentProcess` returns a pseudo-handle that needs no closing, and
    // `counters` is a valid, writable struct whose size is passed as `cb`.
    let result = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) };
    result.ok().map(|()| counters.PeakWorkingSetSize as u64)
}
