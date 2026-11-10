//! ByteForest scanners: turn a volume or folder into a [`bf_core::Tree`].
//!
//! [`MftScanner`] reads the NTFS Master File Table directly (fast; needs administrator rights).
//! [`WalkScanner`] works everywhere by walking directories in parallel. [`scan_auto`] picks
//! the best one for a root. [`list_volumes`] and [`is_elevated`] help the front ends describe
//! the machine.
#![deny(unsafe_code)]

mod auto;
mod cancel;
mod elevation;
mod mft;
mod mft_layout;
mod mft_merge;
mod progress;
mod volumes;
mod walk;

use std::path::PathBuf;

use serde::Serialize;

pub use auto::scan_auto;
pub use cancel::CancelToken;
pub use elevation::is_elevated;
pub use mft::MftScanner;
pub use progress::{Phase, ScanProgress};
pub use volumes::{VolumeSummary, list_volumes};
pub use walk::WalkScanner;

/// Callback that receives progress updates; it may be called from several threads.
pub type ProgressFn<'a> = &'a (dyn Fn(ScanProgress) + Sync);

/// How a tree was produced.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanMethod {
    /// Direct MFT read.
    Mft,
    /// Directory walk.
    Walk,
}

/// Summary of a finished scan.
#[derive(Serialize, Clone, Debug)]
pub struct ScanStats {
    /// Number of files in the tree.
    pub files: u64,
    /// Number of directories in the tree, not counting the root.
    pub dirs: u64,
    /// Total logical size in bytes.
    pub bytes: u64,
    /// Wall-clock scan time in milliseconds.
    pub elapsed_ms: u64,
    /// Scanner that produced the tree.
    pub method: ScanMethod,
    /// Why the preferred scanner was not used, if a fallback happened.
    pub fallback_reason: Option<String>,
    /// MFT records that could not be parsed and were skipped.
    pub skipped_records: u64,
    /// Entries or directories that could not be read.
    pub errors: u64,
}

/// What to scan.
#[derive(Clone, Debug)]
pub enum ScanRoot {
    /// A whole volume, by drive letter.
    Volume(char),
    /// A folder.
    Folder(PathBuf),
}

/// Reasons a scan can fail.
#[derive(thiserror::Error, Debug)]
pub enum ScanError {
    /// The caller cancelled the scan.
    #[error("cancelled")]
    Cancelled,
    /// The root could not be opened for lack of permission.
    #[error("access denied")]
    AccessDenied,
    /// The volume is not NTFS.
    #[error("not NTFS")]
    NotNtfs,
    /// The volume reports NTFS sizes or `$MFT` extents that cannot be read safely.
    #[error("unusable NTFS layout: {0}")]
    BadLayout(String),
    /// An I/O error on the root.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// An NTFS structure could not be parsed.
    #[error(transparent)]
    Ntfs(#[from] bf_ntfs::NtfsError),
}

/// A strategy for building a tree from a [`ScanRoot`].
pub trait Scanner {
    /// Scans `root`, reporting progress and checking `cancel` regularly.
    ///
    /// Returns [`ScanError::Cancelled`] (and no partial tree) if `cancel` fires.
    fn scan(
        &self,
        root: &ScanRoot,
        progress: ProgressFn<'_>,
        cancel: &CancelToken,
    ) -> Result<(bf_core::Tree, ScanStats), ScanError>;
}
