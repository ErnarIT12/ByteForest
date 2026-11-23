//! File categories derived from the file extension, and per-folder extension breakdowns.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::flags::NodeFlags;
use crate::tree::{NONE, NodeId, Tree};

/// Longest extension (in bytes) that is recognised; anything longer counts as "no extension".
pub const MAX_EXT_LEN: usize = 16;

/// Coarse kind of a file, used for colouring and filtering.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum FileCategory {
    Video = 0,
    Archive = 1,
    /// Disk and virtual machine images.
    DiskImage = 2,
    /// Source code, build output and data/config formats.
    CodeBuild = 3,
    /// Audio and images.
    Media = 4,
    /// Operating system files (by extension or the `SYSTEM` attribute).
    System = 5,
    Other = 6,
}

impl FileCategory {
    /// Every category in display order.
    pub const ALL: [FileCategory; 7] = [
        FileCategory::Video,
        FileCategory::Archive,
        FileCategory::DiskImage,
        FileCategory::CodeBuild,
        FileCategory::Media,
        FileCategory::System,
        FileCategory::Other,
    ];

    /// Stable numeric value (`0..=6`).
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Inverse of [`FileCategory::as_u8`]; `None` for values above 6.
    pub fn from_u8(v: u8) -> Option<FileCategory> {
        Self::ALL.get(usize::from(v)).copied()
    }
}

/// The extension of a file name: the text after the last `.`.
///
/// `None` for names without a dot, dotfiles with no other dot (`.gitignore`), a trailing dot,
/// and extensions longer than [`MAX_EXT_LEN`] bytes. The case is preserved.
pub fn extension_of(name: &str) -> Option<&str> {
    let dot = name.rfind('.')?;
    let ext = &name[dot + 1..];
    if dot == 0 || ext.is_empty() || ext.len() > MAX_EXT_LEN {
        return None;
    }
    Some(ext)
}

/// ASCII-lowercased extension in a stack buffer; usable as a hash key without allocating.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ExtKey {
    len: u8,
    buf: [u8; MAX_EXT_LEN],
}

impl ExtKey {
    const NONE: ExtKey = ExtKey {
        len: 0,
        buf: [0; MAX_EXT_LEN],
    };

    fn of(name: &str) -> ExtKey {
        let Some(ext) = extension_of(name) else {
            return ExtKey::NONE;
        };
        let mut buf = [0u8; MAX_EXT_LEN];
        for (dst, src) in buf.iter_mut().zip(ext.bytes()) {
            *dst = src.to_ascii_lowercase();
        }
        ExtKey {
            len: ext.len() as u8,
            buf,
        }
    }

    fn bytes(&self) -> &[u8] {
        &self.buf[..usize::from(self.len)]
    }

    /// ASCII lowercasing keeps UTF-8 valid, so nothing is lost here.
    fn to_string_lossy(self) -> String {
        String::from_utf8_lossy(self.bytes()).into_owned()
    }

    fn category(&self) -> FileCategory {
        match self.bytes() {
            // `ts` is TypeScript far more often than an MPEG transport stream, so it is listed
            // under CodeBuild (video keeps `m2ts`).
            b"mp4" | b"mkv" | b"mov" | b"avi" | b"wmv" | b"webm" | b"m4v" | b"mpg" | b"mpeg"
            | b"flv" | b"m2ts" | b"3gp" => FileCategory::Video,
            b"zip" | b"7z" | b"rar" | b"tar" | b"gz" | b"tgz" | b"bz2" | b"xz" | b"zst"
            | b"lz4" => FileCategory::Archive,
            b"iso" | b"img" | b"vhd" | b"vhdx" | b"vmdk" | b"vdi" | b"qcow2" | b"dmg" | b"wim"
            | b"esd" => FileCategory::DiskImage,
            b"rs" | b"ts" | b"tsx" | b"js" | b"jsx" | b"mjs" | b"cjs" | b"py" | b"c" | b"h"
            | b"cpp" | b"hpp" | b"cs" | b"java" | b"kt" | b"go" | b"rb" | b"php" | b"swift"
            | b"json" | b"toml" | b"yaml" | b"yml" | b"lock" | b"o" | b"obj" | b"a" | b"lib"
            | b"pdb" | b"rlib" | b"rmeta" | b"pyc" | b"class" | b"jar" | b"wasm" | b"map"
            | b"d" | b"node" => FileCategory::CodeBuild,
            b"jpg" | b"jpeg" | b"png" | b"gif" | b"bmp" | b"webp" | b"tif" | b"tiff" | b"heic"
            | b"raw" | b"cr2" | b"nef" | b"psd" | b"svg" | b"mp3" | b"wav" | b"flac" | b"aac"
            | b"ogg" | b"m4a" | b"opus" | b"wma" | b"mid" => FileCategory::Media,
            b"sys" | b"dll" | b"drv" | b"cab" | b"msi" | b"msp" | b"mui" | b"cat" | b"etl"
            | b"evtx" => FileCategory::System,
            _ => FileCategory::Other,
        }
    }
}

/// Category of a file from its name and attributes. Files with the `SYSTEM` attribute are
/// [`FileCategory::System`]; directories and unknown extensions are [`FileCategory::Other`].
/// Extensions are matched case-insensitively without allocating.
pub fn category_of(name: &str, flags: NodeFlags) -> FileCategory {
    if flags.contains(NodeFlags::DIR) {
        FileCategory::Other
    } else if flags.contains(NodeFlags::SYSTEM) {
        FileCategory::System
    } else {
        ExtKey::of(name).category()
    }
}

/// Bytes and file count of one category.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub struct CategoryTotal {
    pub category: FileCategory,
    pub bytes: u64,
    pub files: u64,
}

/// Bytes and file count of one extension.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct ExtTotal {
    /// Lowercase extension without the dot; empty for files without one.
    pub ext: String,
    /// Category from the extension alone (the `SYSTEM` attribute is not considered).
    pub category: FileCategory,
    pub bytes: u64,
    pub files: u64,
}

/// Bytes and file count of the extensions that did not make the top list.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize)]
pub struct OtherExtensions {
    pub bytes: u64,
    pub files: u64,
}

/// Per-category and per-extension totals of a subtree; see [`Tree::extension_breakdown`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct Breakdown {
    /// All seven categories in [`FileCategory::ALL`] order.
    pub categories: Vec<CategoryTotal>,
    /// Largest extensions by bytes (ties by name).
    pub extensions: Vec<ExtTotal>,
    pub other_extensions: OtherExtensions,
    /// Sum of file sizes in the subtree.
    pub total_bytes: u64,
    pub total_files: u64,
}

#[derive(Clone, Copy)]
struct Acc {
    bytes: u64,
    files: u64,
    category: FileCategory,
}

impl Tree {
    /// Sizes of all files under `id` grouped by category and by extension.
    ///
    /// One explicit-stack walk over the subtree; per file there is no allocation (the extension
    /// is lowercased into a stack buffer that serves as the map key). Only files are counted,
    /// so `total_bytes` equals `total(id).size` when directories carry no size of their own.
    /// `None` when `id` is out of range or removed.
    pub fn extension_breakdown(&self, id: NodeId, top_n: usize) -> Option<Breakdown> {
        if id.idx() >= self.len() || self.is_removed(id) {
            return None;
        }
        let mut cats = [(0u64, 0u64); 7];
        let mut exts: HashMap<ExtKey, Acc> = HashMap::new();
        let mut stack = vec![id.0];
        while let Some(n) = stack.pop() {
            let i = n as usize;
            if self.dir_index[i] != NONE {
                let mut c = self.first_child[i];
                while c != NONE {
                    stack.push(c);
                    c = self.next_sibling[c as usize];
                }
                continue;
            }
            let size = self.size[i];
            let key = ExtKey::of(self.name(NodeId(n)));
            let ext_cat = key.category();
            let cat = if self.flags[i].contains(NodeFlags::SYSTEM) {
                FileCategory::System
            } else {
                ext_cat
            };
            let slot = &mut cats[usize::from(cat.as_u8())];
            slot.0 = slot.0.saturating_add(size);
            slot.1 += 1;
            let acc = exts.entry(key).or_insert(Acc {
                bytes: 0,
                files: 0,
                category: ext_cat,
            });
            acc.bytes = acc.bytes.saturating_add(size);
            acc.files += 1;
        }

        let mut list: Vec<(ExtKey, Acc)> = exts.into_iter().collect();
        list.sort_unstable_by(|a, b| {
            b.1.bytes
                .cmp(&a.1.bytes)
                .then_with(|| a.0.bytes().cmp(b.0.bytes()))
        });
        let mut other = OtherExtensions::default();
        for (_, acc) in list.iter().skip(top_n) {
            other.bytes = other.bytes.saturating_add(acc.bytes);
            other.files += acc.files;
        }
        let extensions = list
            .into_iter()
            .take(top_n)
            .map(|(key, acc)| ExtTotal {
                ext: key.to_string_lossy(),
                category: acc.category,
                bytes: acc.bytes,
                files: acc.files,
            })
            .collect();
        let categories: Vec<CategoryTotal> = FileCategory::ALL
            .iter()
            .map(|&category| {
                let (bytes, files) = cats[usize::from(category.as_u8())];
                CategoryTotal {
                    category,
                    bytes,
                    files,
                }
            })
            .collect();
        Some(Breakdown {
            total_bytes: categories
                .iter()
                .fold(0u64, |a, c| a.saturating_add(c.bytes)),
            total_files: categories.iter().map(|c| c.files).sum(),
            categories,
            extensions,
            other_extensions: other,
        })
    }
}
