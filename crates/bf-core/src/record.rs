use crate::flags::NodeFlags;

/// One file-system entry as reported by a scanner.
///
/// `key` and `parent_key` are scanner-defined identifiers (for example MFT record numbers);
/// they only need to be consistent within one scan.
#[derive(Debug, Clone)]
pub struct EntryRecord {
    /// Unique identifier of this entry.
    pub key: u64,
    /// Identifier of the containing directory.
    pub parent_key: u64,
    /// File or directory name (not a path).
    pub name: String,
    /// Logical size in bytes.
    pub size: u64,
    /// Allocated size on disk in bytes.
    pub alloc: u64,
    /// Modification time as a Windows FILETIME.
    pub mtime: i64,
    /// Attribute bits.
    pub flags: NodeFlags,
}
