//! Volume listing and cluster-size lookup through the Win32 API.
#![allow(unsafe_code)]

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use serde::Serialize;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDiskFreeSpaceW, GetLogicalDrives, GetVolumeInformationW,
    GetVolumePathNameW,
};
use windows::core::PCWSTR;

/// Cluster size assumed when the real one cannot be queried.
const DEFAULT_CLUSTER: u64 = 4096;
/// Buffer length (in UTF-16 units) for volume labels, file-system names and paths.
const BUF_LEN: usize = 261;

/// One mounted volume, as shown in the volume picker.
#[derive(Serialize, Clone, Debug)]
pub struct VolumeSummary {
    /// Drive letter, upper case.
    pub letter: char,
    /// Volume label; may be empty.
    pub label: String,
    /// File-system name, e.g. `NTFS`.
    pub fs: String,
    /// Capacity in bytes.
    pub total: u64,
    /// Free space in bytes.
    pub free: u64,
}

/// Lists mounted drive letters. Drives that cannot be queried (empty card readers,
/// disconnected network drives) are skipped.
pub fn list_volumes() -> Vec<VolumeSummary> {
    // SAFETY: takes no arguments and only reads the process's drive bitmask.
    let mask = unsafe { GetLogicalDrives() };
    (0..26u32)
        .filter(|bit| mask & (1 << bit) != 0)
        .filter_map(|bit| query_volume(char::from(b'A' + bit as u8)))
        .collect()
}

fn query_volume(letter: char) -> Option<VolumeSummary> {
    let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain([0]).collect();
    let mut label = [0u16; BUF_LEN];
    let mut fs = [0u16; BUF_LEN];
    // SAFETY: `root` is NUL-terminated and outlives the call; the buffers are valid for
    // writes of their slice lengths; the other out-parameters are `None`.
    unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            Some(&mut label),
            None,
            None,
            None,
            Some(&mut fs),
        )
    }
    .ok()?;

    let mut total = 0u64;
    let mut free = 0u64;
    // SAFETY: `root` is NUL-terminated; `total` and `free` are valid out-pointers.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root.as_ptr()),
            None,
            Some(&mut total),
            Some(&mut free),
        )
    }
    .ok()?;

    Some(VolumeSummary {
        letter,
        label: from_wide(&label),
        fs: from_wide(&fs),
        total,
        free,
    })
}

/// Cluster size in bytes of the volume that contains `path` (4096 if it cannot be queried).
pub(crate) fn cluster_size(path: &Path) -> u64 {
    query_cluster_size(path).unwrap_or(DEFAULT_CLUSTER)
}

fn query_cluster_size(path: &Path) -> Option<u64> {
    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut volume = [0u16; BUF_LEN];
    // SAFETY: `wide_path` is NUL-terminated and outlives the call; `volume` is a valid
    // writable buffer of the given length.
    unsafe { GetVolumePathNameW(PCWSTR(wide_path.as_ptr()), &mut volume) }.ok()?;

    let mut sectors_per_cluster = 0u32;
    let mut bytes_per_sector = 0u32;
    // SAFETY: `volume` is NUL-terminated by the previous call; both out-pointers are valid
    // and the remaining out-parameters are `None`.
    unsafe {
        GetDiskFreeSpaceW(
            PCWSTR(volume.as_ptr()),
            Some(&mut sectors_per_cluster),
            Some(&mut bytes_per_sector),
            None,
            None,
        )
    }
    .ok()?;

    let cluster = u64::from(sectors_per_cluster) * u64::from(bytes_per_sector);
    (cluster > 0).then_some(cluster)
}

/// Converts a NUL-terminated UTF-16 buffer to a `String`.
fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}
