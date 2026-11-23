use bitflags::bitflags;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

bitflags! {
    /// Per-node attribute bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct NodeFlags: u16 {
        /// The node is a directory.
        const DIR = 1;
        /// Hidden attribute.
        const HIDDEN = 2;
        /// System attribute.
        const SYSTEM = 4;
        /// Reparse point (junction, symlink, ...).
        const REPARSE = 8;
        /// File has more than one hard link.
        const HARDLINK = 16;
        /// NTFS-compressed file.
        const COMPRESSED = 32;
        /// Sparse file.
        const SPARSE = 64;
        /// Node does not exist on disk (e.g. `$Orphaned`).
        const SYNTHETIC = 128;
        /// Node was removed from the tree after the scan (moved to the trash).
        const REMOVED = 256;
    }
}

/// Serialized as the raw `u16` bits; unknown bits are dropped on deserialization.
impl Serialize for NodeFlags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for NodeFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from_bits_truncate(u16::deserialize(deserializer)?))
    }
}
