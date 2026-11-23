//! Compact on-disk snapshot of a [`Tree`] (`.bfsnap`), used to compare scans over time.
//!
//! Format v1, all integers little-endian:
//!
//! ```text
//! magic        b"BFSNAP"
//! version      u16 (= 1)
//! reserved     u16 (= 0, ignored on read)
//! created      i64 unix milliseconds
//! label_len    u32 (<= 4096), then label bytes (UTF-8, opaque to this crate)
//! node_count   u32 (>= 1)
//! payload_len  u64 (uncompressed payload size)
//! payload_crc  u32 (CRC-32 of the uncompressed payload)
//! deflate(payload)
//! ```
//!
//! The payload lists the nodes in preorder from the root, skipping removed subtrees, with
//! nodes renumbered `0..node_count`. Per node: `u32 parent_index` (`u32::MAX` for the root,
//! otherwise strictly less than the node's own index), `u16 flags`, varint name length and
//! the UTF-8 name (<= 32 KiB), varint size, varint alloc and zigzag varint mtime.
//!
//! Reading never panics: every length is bounded before it is used, decompression is capped
//! at the declared payload length and the payload is checked against its CRC. The payload is
//! parsed while it streams in, but the tree is only returned once the CRC matched.

use std::io::{self, BufRead, BufReader, Read, Take, Write};

use flate2::Compression;
use flate2::bufread::DeflateDecoder;
use flate2::write::DeflateEncoder;

use crate::builder::TreeBuilder;
use crate::flags::NodeFlags;
use crate::record::EntryRecord;
use crate::tree::{NONE, NodeId, Tree};

const MAGIC: &[u8; 6] = b"BFSNAP";
/// Current format version.
pub const SNAPSHOT_VERSION: u16 = 1;
/// Longest label stored or accepted, in bytes.
pub const MAX_LABEL_LEN: usize = 4096;
/// Longest node name stored or accepted, in bytes.
pub const MAX_NAME_LEN: usize = 32 * 1024;
/// Most nodes a snapshot may declare by default.
pub const MAX_NODE_COUNT: u32 = 100_000_000;
/// Largest uncompressed payload accepted by default, in bytes.
pub const MAX_PAYLOAD_LEN: u64 = 8 * 1024 * 1024 * 1024;
/// Largest uncompressed-to-compressed size ratio [`SnapshotLimits::for_compressed_len`]
/// accepts. Generated 100k-node trees measure about 1.8x (random names) and 12x (a
/// maximally repetitive tree, see the `snapshot_compression_ratio` test); 64 leaves about
/// 5x headroom over the latter while a crafted deflate bomb (up to ~1000x) is refused
/// before anything is decoded.
pub const MAX_COMPRESSION_RATIO: u64 = 64;

/// Caps applied while reading a snapshot, so a small crafted file cannot exhaust memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotLimits {
    /// Most nodes the header may declare.
    pub max_nodes: u32,
    /// Largest uncompressed payload the header may declare, in bytes.
    pub max_payload_len: u64,
}

impl Default for SnapshotLimits {
    fn default() -> Self {
        SnapshotLimits {
            max_nodes: MAX_NODE_COUNT,
            max_payload_len: MAX_PAYLOAD_LEN,
        }
    }
}

impl SnapshotLimits {
    /// Defaults tightened by the size of the compressed file: the payload may not exceed
    /// [`MAX_COMPRESSION_RATIO`] times `compressed_len`.
    pub fn for_compressed_len(compressed_len: u64) -> Self {
        let d = SnapshotLimits::default();
        SnapshotLimits {
            max_nodes: d.max_nodes,
            max_payload_len: d
                .max_payload_len
                .min(compressed_len.saturating_mul(MAX_COMPRESSION_RATIO)),
        }
    }
}

/// Why a snapshot could not be written or read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SnapshotError {
    /// The file does not start with the snapshot magic.
    #[error("not a ByteForest snapshot (bad magic)")]
    BadMagic,
    /// The file was written by an unknown format version.
    #[error("unsupported snapshot version {0}")]
    UnsupportedVersion(u16),
    /// The data ends before the snapshot is complete.
    #[error("snapshot is truncated")]
    Truncated,
    /// The data is structurally invalid.
    #[error("snapshot is corrupt: {0}")]
    Corrupt(&'static str),
    /// The header declares more than the caller's [`SnapshotLimits`] allow.
    #[error("snapshot exceeds the allowed size: {0}")]
    LimitExceeded(&'static str),
    /// The payload does not match its checksum.
    #[error("snapshot checksum mismatch")]
    ChecksumMismatch,
    /// The underlying reader or writer failed.
    #[error("i/o error: {0}")]
    Io(String),
}

fn map_io(e: io::Error) -> SnapshotError {
    match e.kind() {
        io::ErrorKind::UnexpectedEof => SnapshotError::Truncated,
        io::ErrorKind::InvalidData | io::ErrorKind::InvalidInput => {
            SnapshotError::Corrupt("invalid compressed data")
        }
        _ => SnapshotError::Io(e.to_string()),
    }
}

/// A loaded snapshot.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Caller-defined label (for example the scanned target).
    pub label: String,
    /// Creation time in unix milliseconds.
    pub created_unix_ms: i64,
    /// The saved tree. Node ids are preorder indices, not the ids of the original tree.
    pub tree: Tree,
}

// ---------------------------------------------------------------- writing

/// Counts and checksums bytes; discards them.
struct Measure {
    crc: crc32fast::Hasher,
    len: u64,
}

impl Write for Measure {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.crc.update(buf);
        self.len += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn put_varint<W: Write>(w: &mut W, mut v: u64) -> io::Result<()> {
    let mut buf = [0u8; 10];
    let mut i = 0;
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            buf[i] = byte;
            i += 1;
            break;
        }
        buf[i] = byte | 0x80;
        i += 1;
    }
    w.write_all(&buf[..i])
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzigzag(u: u64) -> i64 {
    ((u >> 1) as i64) ^ -((u & 1) as i64)
}

/// Preorder of the live nodes and, per old id, its new index (`NONE` if skipped).
fn preorder(tree: &Tree) -> (Vec<u32>, Vec<u32>) {
    let mut new_index = vec![NONE; tree.len()];
    let mut order = Vec::new();
    let mut stack = vec![0u32];
    while let Some(id) = stack.pop() {
        let i = id as usize;
        if tree.flags[i].contains(NodeFlags::REMOVED) || new_index[i] != NONE {
            continue;
        }
        new_index[i] = order.len() as u32;
        order.push(id);
        let at = stack.len();
        stack.extend(tree.children_ids(NodeId(id)).map(|c| c.0));
        stack[at..].reverse();
    }
    (order, new_index)
}

fn encode_payload<W: Write>(
    tree: &Tree,
    order: &[u32],
    new_index: &[u32],
    out: W,
) -> io::Result<()> {
    let mut w = io::BufWriter::with_capacity(64 * 1024, out);
    for &id in order {
        let i = id as usize;
        let parent = match tree.parent[i] {
            NONE => NONE,
            p => new_index[p as usize],
        };
        w.write_all(&parent.to_le_bytes())?;
        let flags = tree.flags[i] - NodeFlags::REMOVED;
        w.write_all(&flags.bits().to_le_bytes())?;
        let name = truncate_utf8(tree.name(NodeId(id)), MAX_NAME_LEN);
        put_varint(&mut w, name.len() as u64)?;
        w.write_all(name.as_bytes())?;
        put_varint(&mut w, tree.size[i])?;
        put_varint(&mut w, tree.alloc[i])?;
        put_varint(&mut w, zigzag(tree.mtime[i]))?;
    }
    w.flush()
}

/// Writes `tree` as a snapshot. Removed subtrees are left out; `label` is cut to
/// [`MAX_LABEL_LEN`] bytes and over-long names to [`MAX_NAME_LEN`].
pub fn write_snapshot<W: Write>(
    tree: &Tree,
    label: &str,
    created_unix_ms: i64,
    mut w: W,
) -> Result<(), SnapshotError> {
    let io_err = |e: io::Error| SnapshotError::Io(e.to_string());
    let (order, new_index) = preorder(tree);

    // Pass 1 measures the payload so the header can carry its length and checksum.
    let mut measure = Measure {
        crc: crc32fast::Hasher::new(),
        len: 0,
    };
    encode_payload(tree, &order, &new_index, &mut measure).map_err(io_err)?;

    let label = truncate_utf8(label, MAX_LABEL_LEN);
    let mut header = Vec::with_capacity(48 + label.len());
    header.extend_from_slice(MAGIC);
    header.extend_from_slice(&SNAPSHOT_VERSION.to_le_bytes());
    header.extend_from_slice(&0u16.to_le_bytes());
    header.extend_from_slice(&created_unix_ms.to_le_bytes());
    header.extend_from_slice(&(label.len() as u32).to_le_bytes());
    header.extend_from_slice(label.as_bytes());
    header.extend_from_slice(&(order.len() as u32).to_le_bytes());
    header.extend_from_slice(&measure.len.to_le_bytes());
    header.extend_from_slice(&measure.crc.finalize().to_le_bytes());
    w.write_all(&header).map_err(io_err)?;

    // Pass 2 compresses the same bytes.
    let mut enc = DeflateEncoder::new(w, Compression::default());
    encode_payload(tree, &order, &new_index, &mut enc).map_err(io_err)?;
    let mut w = enc.finish().map_err(io_err)?;
    w.flush().map_err(io_err)
}

// ---------------------------------------------------------------- reading

/// Passes bytes through while counting and checksumming them.
struct Hashing<R> {
    inner: R,
    crc: crc32fast::Hasher,
    count: u64,
}

impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.crc.update(&buf[..n]);
        self.count += n as u64;
        Ok(n)
    }
}

fn read_array<R: Read, const N: usize>(r: &mut R) -> Result<[u8; N], SnapshotError> {
    let mut buf = [0u8; N];
    r.read_exact(&mut buf).map_err(map_io)?;
    Ok(buf)
}

fn read_varint<R: Read>(r: &mut R) -> Result<u64, SnapshotError> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let [b] = read_array::<_, 1>(r)?;
        if shift == 63 && b > 1 {
            return Err(SnapshotError::Corrupt("varint overflows 64 bits"));
        }
        value |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
}

/// Reads a snapshot with the default [`SnapshotLimits`]. Never panics, whatever the input.
pub fn read_snapshot<R: Read>(r: R) -> Result<Snapshot, SnapshotError> {
    read_snapshot_with_limits(r, SnapshotLimits::default())
}

/// Reads a snapshot, refusing headers that declare more than `limits` allow. Never panics.
pub fn read_snapshot_with_limits<R: Read>(
    r: R,
    limits: SnapshotLimits,
) -> Result<Snapshot, SnapshotError> {
    let mut file = BufReader::new(r);

    let magic: [u8; 6] = read_array(&mut file)?;
    if &magic != MAGIC {
        return Err(SnapshotError::BadMagic);
    }
    let version = u16::from_le_bytes(read_array(&mut file)?);
    if version != SNAPSHOT_VERSION {
        return Err(SnapshotError::UnsupportedVersion(version));
    }
    let _reserved: [u8; 2] = read_array(&mut file)?;
    let created_unix_ms = i64::from_le_bytes(read_array(&mut file)?);
    let label_len = u32::from_le_bytes(read_array(&mut file)?) as usize;
    if label_len > MAX_LABEL_LEN {
        return Err(SnapshotError::Corrupt("label too long"));
    }
    let mut label = vec![0u8; label_len];
    file.read_exact(&mut label).map_err(map_io)?;
    let label =
        String::from_utf8(label).map_err(|_| SnapshotError::Corrupt("label is not UTF-8"))?;
    let node_count = u32::from_le_bytes(read_array(&mut file)?);
    let payload_len = u64::from_le_bytes(read_array(&mut file)?);
    let payload_crc = u32::from_le_bytes(read_array(&mut file)?);

    if payload_len > limits.max_payload_len {
        return Err(SnapshotError::LimitExceeded("payload too large"));
    }
    if node_count == 0 {
        return Err(SnapshotError::Corrupt("snapshot has no root"));
    }
    if node_count > limits.max_nodes {
        return Err(SnapshotError::LimitExceeded("too many nodes"));
    }
    if u64::from(node_count) > payload_len / 4 {
        return Err(SnapshotError::Corrupt("node count exceeds payload size"));
    }

    // The decompressed stream is capped one byte past the declared length, which catches
    // both decompression bombs and a payload longer than declared.
    let limited: Take<DeflateDecoder<&mut BufReader<R>>> =
        DeflateDecoder::new(&mut file).take(payload_len + 1);
    let mut hashing = Hashing {
        inner: limited,
        crc: crc32fast::Hasher::new(),
        count: 0,
    };
    let tree = {
        let mut payload = BufReader::with_capacity(64 * 1024, &mut hashing);
        let tree = parse_nodes(&mut payload, node_count)?;
        // Drain the rest so the CRC covers everything and the deflate stream is finished.
        io::copy(&mut payload, &mut io::sink()).map_err(map_io)?;
        tree
    };
    if hashing.count != payload_len {
        return Err(SnapshotError::Corrupt("payload length mismatch"));
    }
    if hashing.crc.finalize() != payload_crc {
        return Err(SnapshotError::ChecksumMismatch);
    }

    if !file.fill_buf().map_err(map_io)?.is_empty() {
        return Err(SnapshotError::Corrupt("trailing data after snapshot"));
    }
    Ok(Snapshot {
        label,
        created_unix_ms,
        tree,
    })
}

/// Parses the node records. The result is only returned to callers after the CRC check.
fn parse_nodes<R: Read>(r: &mut R, node_count: u32) -> Result<Tree, SnapshotError> {
    let mut builder: Option<TreeBuilder> = None;
    // One bit per node: is it a directory?
    let mut is_dir: Vec<u64> = Vec::new();
    let mut name_buf: Vec<u8> = Vec::new();

    for i in 0..node_count {
        let parent = u32::from_le_bytes(read_array(r)?);
        let flags =
            NodeFlags::from_bits_truncate(u16::from_le_bytes(read_array(r)?)) - NodeFlags::REMOVED;
        let name_len = read_varint(r)?;
        if name_len > MAX_NAME_LEN as u64 {
            return Err(SnapshotError::Corrupt("node name too long"));
        }
        name_buf.clear();
        name_buf.resize(name_len as usize, 0);
        r.read_exact(&mut name_buf).map_err(map_io)?;
        let name = std::str::from_utf8(&name_buf)
            .map_err(|_| SnapshotError::Corrupt("node name is not UTF-8"))?;
        let size = read_varint(r)?;
        let alloc = read_varint(r)?;
        let mtime = unzigzag(read_varint(r)?);

        if i == 0 {
            if parent != NONE {
                return Err(SnapshotError::Corrupt("first node is not a root"));
            }
            if !flags.contains(NodeFlags::DIR) {
                return Err(SnapshotError::Corrupt("root is not a directory"));
            }
            builder = Some(TreeBuilder::new(0, name));
        } else {
            if parent >= i {
                return Err(SnapshotError::Corrupt("parent index not before child"));
            }
            let parent_is_dir = is_dir
                .get(parent as usize / 64)
                .is_some_and(|w| (w >> (parent % 64)) & 1 == 1);
            if !parent_is_dir {
                return Err(SnapshotError::Corrupt("parent is not a directory"));
            }
            let Some(b) = builder.as_mut() else {
                return Err(SnapshotError::Corrupt("missing root"));
            };
            b.push(EntryRecord {
                key: u64::from(i),
                parent_key: u64::from(parent),
                name: name.to_string(),
                size,
                alloc,
                mtime,
                flags,
            });
            if b.len() != i as usize + 1 {
                return Err(SnapshotError::Corrupt("tree too large"));
            }
        }
        if i as usize / 64 >= is_dir.len() {
            is_dir.push(0);
        }
        if flags.contains(NodeFlags::DIR) {
            is_dir[i as usize / 64] |= 1 << (i % 64);
        }
    }
    builder
        .map(TreeBuilder::finish)
        .ok_or(SnapshotError::Corrupt("snapshot has no root"))
}
