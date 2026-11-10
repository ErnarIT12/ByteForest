//! Walking and decoding the attributes of a FILE record.
//!
//! The functions here return plain `NtfsError`s (usually `Truncated`) for a
//! malformed field. `parse_record` turns any error that happens inside an
//! attribute into `BadAttribute(offset of that attribute)`.

use crate::{NtfsError, bytes_at, i64_at, u16_at, u32_at, u64_at};

pub(crate) const TYPE_STANDARD_INFORMATION: u32 = 0x10;
pub(crate) const TYPE_FILE_NAME: u32 = 0x30;
pub(crate) const TYPE_DATA: u32 = 0x80;

/// Marks the end of the attribute list.
const END_MARKER: u32 = 0xFFFF_FFFF;
/// Bytes every attribute header has: type, length, form, name length, flags.
const COMMON_HEADER_LEN: usize = 16;
/// Header of a non-resident attribute without a compressed size.
const NONRESIDENT_HEADER_LEN: usize = 64;
/// Header of a non-resident attribute that has the compressed size field.
const COMPRESSED_HEADER_LEN: usize = 72;
/// Attribute flags: compressed and sparse.
const FLAG_COMPRESSED: u16 = 0x0001;
const FLAG_SPARSE: u16 = 0x8000;
/// A file reference keeps the record number in its low 48 bits.
const REFERENCE_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

/// One attribute: its type, where it starts in the record, and all its bytes.
pub(crate) struct Attr<'a> {
    pub type_code: u32,
    /// Offset of the attribute inside the record (for error reporting).
    pub offset: usize,
    /// The whole attribute, header included. At least `COMMON_HEADER_LEN` long.
    pub bytes: &'a [u8],
}

/// Iterator over the attributes in `region`, starting at `first`.
///
/// `region` must already be limited to the used part of the record. After an
/// error the iterator stops.
pub(crate) struct Attrs<'a> {
    region: &'a [u8],
    pos: usize,
    done: bool,
}

impl<'a> Attrs<'a> {
    pub fn new(region: &'a [u8], first: usize) -> Self {
        Self {
            region,
            pos: first,
            done: false,
        }
    }

    /// Reads the attribute at `self.pos`; `None` means the end marker.
    fn step(&mut self) -> Result<Option<Attr<'a>>, NtfsError> {
        let offset = self.pos;
        let bad = |_: NtfsError| NtfsError::BadAttribute(offset);

        let type_code = u32_at(self.region, offset).map_err(bad)?;
        if type_code == END_MARKER {
            return Ok(None);
        }
        let length = u32_at(self.region, offset + 4).map_err(bad)?;
        // `length == 0` would never advance; a too-short one has no header.
        let length = usize::try_from(length).map_err(|_| NtfsError::BadAttribute(offset))?;
        if length < COMMON_HEADER_LEN {
            return Err(NtfsError::BadAttribute(offset));
        }
        let bytes = bytes_at(self.region, offset, length).map_err(bad)?;
        self.pos = offset + length;
        Ok(Some(Attr {
            type_code,
            offset,
            bytes,
        }))
    }
}

impl<'a> Iterator for Attrs<'a> {
    type Item = Result<Attr<'a>, NtfsError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.step() {
            Ok(Some(attr)) => Some(Ok(attr)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

impl Attr<'_> {
    fn is_non_resident(&self) -> bool {
        self.bytes[8] != 0
    }

    /// Length of the attribute name in UTF-16 units (0 = unnamed).
    fn name_len(&self) -> u8 {
        self.bytes[9]
    }

    fn flags(&self) -> Result<u16, NtfsError> {
        u16_at(self.bytes, 12)
    }

    /// The value of a resident attribute.
    fn resident_value(&self) -> Result<&[u8], NtfsError> {
        if self.is_non_resident() {
            return Err(NtfsError::Truncated);
        }
        let len = usize::try_from(u32_at(self.bytes, 16)?).map_err(|_| NtfsError::Truncated)?;
        let off = usize::from(u16_at(self.bytes, 20)?);
        bytes_at(self.bytes, off, len)
    }
}

/// Fields of `$STANDARD_INFORMATION` that we need.
pub(crate) struct StdInfo {
    pub mtime: i64,
    pub file_attributes: u32,
}

pub(crate) fn parse_std_info(attr: &Attr) -> Result<StdInfo, NtfsError> {
    let value = attr.resident_value()?;
    Ok(StdInfo {
        mtime: i64_at(value, 8)?,
        file_attributes: u32_at(value, 32)?,
    })
}

/// Fields of `$FILE_NAME` that we need.
pub(crate) struct RawFileName {
    pub parent: u64,
    pub namespace: u8,
    pub name: String,
}

pub(crate) fn parse_file_name(attr: &Attr) -> Result<RawFileName, NtfsError> {
    let value = attr.resident_value()?;
    let parent = u64_at(value, 0)? & REFERENCE_MASK;
    let name_len = usize::from(bytes_at(value, 64, 1)?[0]);
    let namespace = bytes_at(value, 65, 1)?[0];
    let (pairs, _) = bytes_at(value, 66, 2 * name_len)?.as_chunks::<2>();
    let units: Vec<u16> = pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    Ok(RawFileName {
        parent,
        namespace,
        name: String::from_utf16_lossy(&units),
    })
}

/// What one `$DATA` attribute contributes to a file.
pub(crate) struct DataStream<'a> {
    /// True for an alternate data stream (`:name`).
    pub named: bool,
    /// Logical size in bytes.
    pub size: u64,
    /// Bytes allocated on disk (0 for resident data).
    pub alloc: u64,
    /// The encoded run list, only for the first piece (VCN 0) of a
    /// non-resident stream.
    pub runs: Option<&'a [u8]>,
}

/// Reads an `i64` size field that must not be negative.
fn size_at(bytes: &[u8], off: usize) -> Result<u64, NtfsError> {
    u64::try_from(i64_at(bytes, off)?).map_err(|_| NtfsError::Truncated)
}

pub(crate) fn parse_data<'a>(attr: &Attr<'a>) -> Result<DataStream<'a>, NtfsError> {
    let named = attr.name_len() != 0;
    if !attr.is_non_resident() {
        let value = attr.resident_value()?;
        return Ok(DataStream {
            named,
            size: value.len() as u64,
            alloc: 0,
            runs: None,
        });
    }

    // A stream can be split into several non-resident pieces (in extension
    // records, or when the runs do not fit). Only the piece that starts at
    // VCN 0 has meaningful size fields; the others contribute nothing.
    if u64_at(attr.bytes, 16)? != 0 {
        return Ok(DataStream {
            named,
            size: 0,
            alloc: 0,
            runs: None,
        });
    }
    if attr.bytes.len() < NONRESIDENT_HEADER_LEN {
        return Err(NtfsError::Truncated);
    }
    let runs_off = usize::from(u16_at(attr.bytes, 32)?);
    let mut alloc = size_at(attr.bytes, 40)?;
    let size = size_at(attr.bytes, 48)?;
    // Compressed and sparse streams: the allocated size is the compressed
    // size, a field that exists only in the longer header (runs start after it).
    let header_len = if attr.flags()? & (FLAG_COMPRESSED | FLAG_SPARSE) != 0 {
        alloc = size_at(attr.bytes, 64)?;
        COMPRESSED_HEADER_LEN
    } else {
        NONRESIDENT_HEADER_LEN
    };
    if runs_off < header_len {
        return Err(NtfsError::Truncated);
    }
    let runs = attr.bytes.get(runs_off..).ok_or(NtfsError::Truncated)?;
    Ok(DataStream {
        named,
        size,
        alloc,
        runs: Some(runs),
    })
}
