//! Duplicate files: confirms the size groups of [`bf_core::Tree::duplicate_candidates`] by
//! reading the files, in three steps:
//!
//! 1. the candidates already share an exact size (no disk access);
//! 2. a partial BLAKE3 hash of the first and last [`PARTIAL_CHUNK`] bytes splits most
//!    non-identical files apart after a few kilobytes of reading;
//! 3. a full BLAKE3 hash confirms that the remaining files have identical contents.
//!
//! The files are read on a small dedicated thread pool ([`default_hash_threads`]) instead of
//! every core, so a spinning disk or an antivirus scanner is not flooded with parallel reads.
//! Files that cannot be read, changed since the scan, turned out to be hard links or links,
//! or are cloud placeholders whose contents are not on the disk are skipped and counted; they
//! never fail the search.

use std::collections::HashMap;
use std::fs::{File, Metadata};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use bf_core::{
    DuplicateCandidates, DuplicateGroup, NodeFlags, NodeId, Tree, sort_duplicate_groups,
};
use rayon::prelude::*;
use serde::Serialize;

use crate::{CancelToken, ScanError};

/// Bytes hashed from each end of a file in the partial step.
pub const PARTIAL_CHUNK: u64 = 16 * 1024;
/// Fewest reader threads [`default_hash_threads`] picks.
pub const MIN_HASH_THREADS: usize = 4;
/// Most reader threads [`default_hash_threads`] picks.
pub const MAX_HASH_THREADS: usize = 8;
/// Read buffer of the full hash.
const READ_BUFFER: usize = 256 * 1024;
/// Shortest pause between two progress reports, in milliseconds.
const PROGRESS_INTERVAL_MS: u64 = 100;

/// Reader threads for hashing: the number of cores, kept between [`MIN_HASH_THREADS`] and
/// [`MAX_HASH_THREADS`].
pub fn default_hash_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(MIN_HASH_THREADS, |n| n.get())
        .clamp(MIN_HASH_THREADS, MAX_HASH_THREADS)
}

/// How the files are read.
#[derive(Clone, Debug)]
pub struct HashOptions {
    /// Reader threads (at least 1).
    pub threads: usize,
}

impl Default for HashOptions {
    fn default() -> Self {
        Self {
            threads: default_hash_threads(),
        }
    }
}

/// Step of a duplicate search.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashPhase {
    /// Hashing the beginning and end of every candidate.
    Partial,
    /// Hashing the whole contents of the files that are still alike.
    Full,
}

/// Progress of a duplicate search. Counters restart at every phase.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HashProgress {
    pub phase: HashPhase,
    /// Files finished in this phase.
    pub files_done: u64,
    /// Files to read in this phase.
    pub files_total: u64,
    /// Bytes read in this phase.
    pub bytes_done: u64,
    /// Bytes to read in this phase.
    pub bytes_total: u64,
}

/// Callback that receives progress; it is called from the reader threads.
pub type HashProgressFn<'a> = &'a (dyn Fn(HashProgress) + Sync);

/// Result of [`find_duplicates`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DuplicateOutcome {
    /// Confirmed groups, most reclaimable space first.
    pub groups: Vec<DuplicateGroup>,
    /// Candidates that were skipped: unreadable, changed since the scan, links, or cloud files
    /// whose contents are not on the disk.
    pub skipped: u64,
    /// Bytes read from the disk.
    pub bytes_read: u64,
}

/// One file that is still in the running.
#[derive(Clone)]
struct Item {
    id: NodeId,
    size: u64,
    path: PathBuf,
}

/// What reading one file gave.
enum Read1 {
    /// The hash, and whether it covers the whole file.
    Hashed([u8; 32], bool),
    Skipped,
    Cancelled,
}

/// Shared counters and throttled reporting of one phase.
struct Meter<'a> {
    phase: HashPhase,
    files_total: u64,
    bytes_total: u64,
    files_done: AtomicU64,
    bytes_done: AtomicU64,
    bytes_read: &'a AtomicU64,
    started: Instant,
    last_report_ms: AtomicU64,
    progress: HashProgressFn<'a>,
}

impl<'a> Meter<'a> {
    fn new(
        phase: HashPhase,
        files_total: u64,
        bytes_total: u64,
        bytes_read: &'a AtomicU64,
        progress: HashProgressFn<'a>,
    ) -> Self {
        let meter = Self {
            phase,
            files_total,
            bytes_total,
            files_done: AtomicU64::new(0),
            bytes_done: AtomicU64::new(0),
            bytes_read,
            started: Instant::now(),
            last_report_ms: AtomicU64::new(0),
            progress,
        };
        meter.report();
        meter
    }

    fn snapshot(&self) -> HashProgress {
        HashProgress {
            phase: self.phase,
            files_done: self.files_done.load(Ordering::Relaxed),
            files_total: self.files_total,
            bytes_done: self
                .bytes_done
                .load(Ordering::Relaxed)
                .min(self.bytes_total),
            bytes_total: self.bytes_total,
        }
    }

    fn report(&self) {
        (self.progress)(self.snapshot());
    }

    /// Reports at most every [`PROGRESS_INTERVAL_MS`]; only one thread wins each slot.
    fn maybe_report(&self) {
        let now = self.started.elapsed().as_millis() as u64;
        let last = self.last_report_ms.load(Ordering::Relaxed);
        if now.saturating_sub(last) >= PROGRESS_INTERVAL_MS
            && self
                .last_report_ms
                .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            self.report();
        }
    }

    fn add_bytes(&self, n: u64) {
        self.bytes_done.fetch_add(n, Ordering::Relaxed);
        self.bytes_read.fetch_add(n, Ordering::Relaxed);
        self.maybe_report();
    }

    /// Marks one file finished; `planned` is what the phase expected to read from it.
    fn file_done(&self, planned: u64, read: u64) {
        // A skipped file still counts toward the planned total so the bar reaches the end.
        if planned > read {
            self.bytes_done.fetch_add(planned - read, Ordering::Relaxed);
        }
        self.files_done.fetch_add(1, Ordering::Relaxed);
        self.maybe_report();
    }
}

/// The native path of a node, built only from the tree; `None` for entries that have no usable
/// path (synthetic nodes, names that are not valid on the file system or lost characters
/// while decoding).
pub fn native_path_of(tree: &Tree, id: NodeId) -> Option<PathBuf> {
    let mut chain: Vec<NodeId> = std::iter::successors(Some(id), |&n| tree.parent(n)).collect();
    chain.reverse();
    let (&root, rest) = chain.split_first()?;
    let root_name = tree.name(root);
    if root_name.contains('\u{fffd}') || tree.flags(root).contains(NodeFlags::SYNTHETIC) {
        return None;
    }
    #[cfg(windows)]
    let mut path = if matches!(root_name.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic())
    {
        PathBuf::from(format!("{root_name}\\"))
    } else {
        PathBuf::from(root_name)
    };
    #[cfg(not(windows))]
    let mut path = PathBuf::from(root_name);
    if !path.is_absolute() {
        return None;
    }
    for &node in rest {
        let name = tree.name(node);
        if tree.flags(node).contains(NodeFlags::SYNTHETIC)
            || name.contains('\u{fffd}')
            || (cfg!(windows) && name.contains(':'))
        {
            return None;
        }
        let mut parts = Path::new(name).components();
        match (parts.next(), parts.next()) {
            (Some(Component::Normal(part)), None) if part == name => path.push(name),
            _ => return None,
        }
    }
    Some(path)
}

/// Confirms which candidates have identical contents.
///
/// `path_of` gives the native path of a node, or `None` to skip it. Returns
/// [`ScanError::Cancelled`] (and no partial result) once `cancel` fires.
pub fn find_duplicates(
    candidates: &DuplicateCandidates,
    path_of: &(dyn Fn(NodeId) -> Option<PathBuf> + Sync),
    options: &HashOptions,
    progress: HashProgressFn<'_>,
    cancel: &CancelToken,
) -> Result<DuplicateOutcome, ScanError> {
    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(options.threads.max(1))
        .thread_name(|i| format!("bf-hash-{i}"))
        .build()
        .map_err(|e| ScanError::Io(io::Error::other(e)))?;

    let mut skipped = 0u64;
    let mut items = Vec::with_capacity(candidates.files as usize);
    for group in &candidates.groups {
        for &id in &group.ids {
            match path_of(id) {
                Some(path) => items.push(Item {
                    id,
                    size: group.size,
                    path,
                }),
                None => skipped += 1,
            }
        }
    }
    let bytes_read = AtomicU64::new(0);

    // Step 2: partial hash of every candidate whose size still has company.
    let items = keep_shared_sizes(items);
    if items.is_empty() {
        return Ok(DuplicateOutcome {
            groups: Vec::new(),
            skipped,
            bytes_read: 0,
        });
    }
    let planned = |item: &Item| item.size.min(2 * PARTIAL_CHUNK);
    let meter = Meter::new(
        HashPhase::Partial,
        items.len() as u64,
        items.iter().map(planned).sum(),
        &bytes_read,
        progress,
    );
    let partial: Vec<(Item, Read1)> = pool.install(|| {
        items
            .into_par_iter()
            .map(|item| {
                let result = partial_hash(&item, &meter, cancel);
                (item, result)
            })
            .collect()
    });
    meter.report();
    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }

    let mut confirmed: HashMap<(u64, [u8; 32]), Vec<NodeId>> = HashMap::new();
    let mut by_partial: HashMap<(u64, [u8; 32]), Vec<Item>> = HashMap::new();
    for (item, result) in partial {
        match result {
            // Small files are hashed whole already; they skip step 3.
            Read1::Hashed(hash, true) => confirmed
                .entry((item.size, hash))
                .or_default()
                .push(item.id),
            Read1::Hashed(hash, false) => {
                by_partial.entry((item.size, hash)).or_default().push(item)
            }
            Read1::Skipped => skipped += 1,
            Read1::Cancelled => return Err(ScanError::Cancelled),
        }
    }

    // Step 3: full hash of the files whose partial hashes still match another file.
    let full_items: Vec<Item> = by_partial
        .into_values()
        .filter(|group| group.len() >= 2)
        .flatten()
        .collect();
    if !full_items.is_empty() {
        let meter = Meter::new(
            HashPhase::Full,
            full_items.len() as u64,
            full_items.iter().map(|i| i.size).sum(),
            &bytes_read,
            progress,
        );
        let full: Vec<(Item, Read1)> = pool.install(|| {
            full_items
                .into_par_iter()
                .map(|item| {
                    let result = full_hash(&item, &meter, cancel);
                    (item, result)
                })
                .collect()
        });
        meter.report();
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }
        for (item, result) in full {
            match result {
                Read1::Hashed(hash, _) => confirmed
                    .entry((item.size, hash))
                    .or_default()
                    .push(item.id),
                Read1::Skipped => skipped += 1,
                Read1::Cancelled => return Err(ScanError::Cancelled),
            }
        }
    }

    let mut groups: Vec<DuplicateGroup> = confirmed
        .into_iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .map(|((size, hash), mut ids)| {
            ids.sort_unstable_by_key(|id| id.0);
            DuplicateGroup { size, hash, ids }
        })
        .collect();
    sort_duplicate_groups(&mut groups);
    Ok(DuplicateOutcome {
        groups,
        skipped,
        bytes_read: bytes_read.load(Ordering::Relaxed),
    })
}

/// Drops items whose size no other item shares (their partner had no path).
fn keep_shared_sizes(items: Vec<Item>) -> Vec<Item> {
    let mut count: HashMap<u64, usize> = HashMap::new();
    for item in &items {
        *count.entry(item.size).or_default() += 1;
    }
    items
        .into_iter()
        .filter(|item| count[&item.size] >= 2)
        .collect()
}

/// Opens a candidate for reading after checking it is still the plain file the scan saw.
fn open_checked(item: &Item) -> Option<File> {
    let meta = std::fs::symlink_metadata(&item.path).ok()?;
    if !meta.file_type().is_file() || meta.len() != item.size || not_on_disk(&meta) {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.nlink() > 1 {
            return None;
        }
    }
    let file = File::open(&item.path).ok()?;
    #[cfg(windows)]
    if windows_link_count(&file).is_none_or(|links| links > 1) {
        return None;
    }
    // The file may have been swapped between the two calls.
    let opened = file.metadata().ok()?;
    (opened.is_file() && opened.len() == item.size).then_some(file)
}

/// Cloud placeholders (OneDrive, iCloud) whose contents would have to be downloaded first.
fn not_on_disk(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const OFFLINE: u32 = 0x1000;
        const RECALL_ON_OPEN: u32 = 0x4_0000;
        const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
        meta.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        const SF_DATALESS: u32 = 0x4000_0000;
        meta.st_flags() & SF_DATALESS != 0
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = meta;
        false
    }
}

/// Number of hard links of an open file.
#[cfg(windows)]
#[allow(unsafe_code)]
fn windows_link_count(file: &File) -> Option<u32> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: the handle belongs to `file`, which outlives the call, and `info` is a valid,
    // writable out-parameter.
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }.ok()?;
    Some(info.nNumberOfLinks)
}

/// Reads up to `buf.len()` bytes, stopping early only at the end of the file.
fn read_full(file: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// Hash of the first and last [`PARTIAL_CHUNK`] bytes; covers the whole file when it is at
/// most twice that long.
fn partial_hash(item: &Item, meter: &Meter<'_>, cancel: &CancelToken) -> Read1 {
    let planned = item.size.min(2 * PARTIAL_CHUNK);
    if cancel.is_cancelled() {
        return Read1::Cancelled;
    }
    let Some(mut file) = open_checked(item) else {
        meter.file_done(planned, 0);
        return Read1::Skipped;
    };
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; PARTIAL_CHUNK as usize];
    let whole = item.size <= 2 * PARTIAL_CHUNK;
    let result = (|| -> io::Result<u64> {
        if whole {
            let mut all = vec![0u8; item.size as usize];
            let n = read_full(&mut file, &mut all)?;
            if n as u64 != item.size || read_full(&mut file, &mut buf[..1])? != 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            hasher.update(&all);
            return Ok(n as u64);
        }
        let head = read_full(&mut file, &mut buf)?;
        if head as u64 != PARTIAL_CHUNK {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        hasher.update(&buf);
        file.seek(SeekFrom::Start(item.size - PARTIAL_CHUNK))?;
        let tail = read_full(&mut file, &mut buf)?;
        if tail as u64 != PARTIAL_CHUNK {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        hasher.update(&buf);
        Ok(2 * PARTIAL_CHUNK)
    })();
    match result {
        Ok(read) => {
            meter.add_bytes(read);
            meter.file_done(planned, read);
            Read1::Hashed(*hasher.finalize().as_bytes(), whole)
        }
        Err(_) => {
            meter.file_done(planned, 0);
            Read1::Skipped
        }
    }
}

/// Hash of the whole file; skipped if its length no longer matches the scan.
fn full_hash(item: &Item, meter: &Meter<'_>, cancel: &CancelToken) -> Read1 {
    if cancel.is_cancelled() {
        return Read1::Cancelled;
    }
    let Some(mut file) = open_checked(item) else {
        meter.file_done(item.size, 0);
        return Read1::Skipped;
    };
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; READ_BUFFER];
    let mut read = 0u64;
    loop {
        if cancel.is_cancelled() {
            meter.file_done(item.size, read);
            return Read1::Cancelled;
        }
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                read += n as u64;
                if read > item.size {
                    break;
                }
                hasher.update(&buf[..n]);
                meter.add_bytes(n as u64);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => {
                meter.file_done(item.size, read);
                return Read1::Skipped;
            }
        }
    }
    meter.file_done(item.size, read.min(item.size));
    if read != item.size {
        return Read1::Skipped;
    }
    Read1::Hashed(*hasher.finalize().as_bytes(), true)
}
