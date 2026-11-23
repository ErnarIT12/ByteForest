//! Local mount points and their capacity.
#![allow(unsafe_code)]

use std::ffi::CStr;
use std::mem::{MaybeUninit, size_of};
use std::path::Path;

use serde::Serialize;

/// One locally mounted volume.
#[derive(Serialize, Clone, Debug)]
pub struct VolumeSummary {
    pub path: String,
    pub label: String,
    pub fs: String,
    pub total: u64,
    pub free: u64,
}

/// The data volume and local external volumes. A separate read-only system volume
/// is omitted when the data volume is mounted.
pub fn list_volumes() -> Vec<VolumeSummary> {
    let Some(mounts) = mount_stats() else {
        return Vec::new();
    };
    let mut volumes: Vec<_> = mounts
        .iter()
        .filter(|mount| mount.f_flags & (libc::MNT_LOCAL as u32) != 0)
        .filter_map(|mount| {
            let path = mount_string(&mount.f_mntonname).to_str().ok()?;
            if path != "/" && path != "/System/Volumes/Data" && !path.starts_with("/Volumes/") {
                return None;
            }
            let label = match path {
                "/" => "Startup volume",
                "/System/Volumes/Data" => "Mac data volume",
                _ => Path::new(path).file_name()?.to_str()?,
            };
            let block_size = u64::from(mount.f_bsize);
            Some(VolumeSummary {
                path: path.to_owned(),
                label: label.to_owned(),
                fs: mount_string(&mount.f_fstypename)
                    .to_string_lossy()
                    .into_owned(),
                total: mount.f_blocks.saturating_mul(block_size),
                free: mount.f_bavail.saturating_mul(block_size),
            })
        })
        .collect();
    if volumes
        .iter()
        .any(|volume| volume.path == "/System/Volumes/Data")
    {
        volumes.retain(|volume| volume.path != "/");
    }
    volumes.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    volumes
}

fn mount_string(bytes: &[libc::c_char]) -> &CStr {
    // SAFETY: c_char occupies one byte. Keep interpretation within the fixed-size field.
    let bytes = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<u8>(), bytes.len()) };
    CStr::from_bytes_until_nul(bytes).unwrap_or(c"")
}

fn mount_stats() -> Option<Vec<libc::statfs>> {
    // SAFETY: a null buffer is the documented count-only query.
    let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    if count <= 0 {
        return None;
    }
    let mut capacity = usize::try_from(count).ok()?.checked_add(8)?;
    loop {
        let bytes = capacity.checked_mul(size_of::<libc::statfs>())?;
        let bytes = libc::c_int::try_from(bytes).ok()?;
        let mut buffer = Vec::<MaybeUninit<libc::statfs>>::with_capacity(capacity);
        // SAFETY: the allocation is writable for exactly the byte length given to getfsstat.
        let actual =
            unsafe { libc::getfsstat(buffer.as_mut_ptr().cast(), bytes, libc::MNT_NOWAIT) };
        let actual = usize::try_from(actual).ok()?;
        if actual < capacity {
            // SAFETY: getfsstat initialized the returned number of records.
            unsafe { buffer.set_len(actual) };
            return Some(
                buffer
                    .into_iter()
                    .map(|item| unsafe { item.assume_init() })
                    .collect(),
            );
        }
        capacity = capacity.checked_mul(2)?;
    }
}
