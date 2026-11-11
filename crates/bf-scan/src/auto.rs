//! Picks the fastest scanner that can work on a root.

use bf_core::Tree;

use crate::volumes::filesystem_name;
use crate::{
    CancelToken, MftScanner, ProgressFn, ScanError, ScanRoot, ScanStats, Scanner, WalkScanner,
    is_elevated,
};

/// Scans `root` with [`MftScanner`] when possible, else with [`WalkScanner`].
///
/// A volume is read through its MFT when the process is elevated and the volume is NTFS. If
/// that is not the case, or the MFT scan fails with anything but [`ScanError::Cancelled`],
/// the walker runs and [`ScanStats::fallback_reason`] says why. Folders always use the
/// walker, with no fallback reason.
pub fn scan_auto(
    root: &ScanRoot,
    progress: ProgressFn<'_>,
    cancel: &CancelToken,
) -> Result<(Tree, ScanStats), ScanError> {
    let ScanRoot::Volume(letter) = root else {
        return WalkScanner.scan(root, progress, cancel);
    };

    let fs = filesystem_name(*letter);
    let reason = match mft_unavailable_reason(fs.as_deref(), is_elevated()) {
        Some(reason) => reason,
        None => match MftScanner.scan(root, progress, cancel) {
            Ok(result) => return Ok(result),
            Err(ScanError::Cancelled) => return Err(ScanError::Cancelled),
            Err(err) => format!("fast scan failed: {err}"),
        },
    };

    let (tree, mut stats) = WalkScanner.scan(root, progress, cancel)?;
    stats.fallback_reason = Some(reason);
    Ok((tree, stats))
}

/// Why the MFT scanner cannot be used on a volume, or `None` if it can.
///
/// `fs` is the volume's file-system name (`None` if it could not be queried).
fn mft_unavailable_reason(fs: Option<&str>, elevated: bool) -> Option<String> {
    match fs {
        None => Some("file system could not be determined".to_string()),
        Some(fs) if !fs.eq_ignore_ascii_case("NTFS") => {
            Some(format!("filesystem is {fs}, not NTFS"))
        }
        Some(_) if !elevated => Some("administrator rights required for fast scan".to_string()),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::mft_unavailable_reason;

    #[test]
    fn ntfs_and_elevated_uses_mft() {
        assert_eq!(mft_unavailable_reason(Some("NTFS"), true), None);
    }

    #[test]
    fn not_elevated_needs_admin() {
        assert_eq!(
            mft_unavailable_reason(Some("NTFS"), false).as_deref(),
            Some("administrator rights required for fast scan")
        );
    }

    #[test]
    fn other_file_systems_are_named() {
        // The file system is reported first: elevation would not help.
        for elevated in [true, false] {
            assert_eq!(
                mft_unavailable_reason(Some("exFAT"), elevated).as_deref(),
                Some("filesystem is exFAT, not NTFS")
            );
        }
    }

    #[test]
    fn unknown_file_system_falls_back() {
        assert!(mft_unavailable_reason(None, true).is_some());
    }
}
