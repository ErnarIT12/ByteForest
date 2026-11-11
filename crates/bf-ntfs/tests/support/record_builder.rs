//! Synthetic NTFS FILE-record generator for tests.
//!
//! Builds a record the way NTFS stores it on disk: header, update sequence
//! array (USA), attributes, end marker, and finally the "protection" step that
//! copies each sector's last two bytes into the USA and overwrites them with
//! the update sequence number.
//!
//! Attribute builders (`std_info`, `file_name`, `data_*`) push serialized
//! attributes into `attrs`; each one is a small function below that writes the
//! attribute header followed by its value.

#![allow(dead_code)]

/// Update sequence number written into every sector tail.
pub const USN: u16 = 0x0001;
/// Offset of the update sequence array inside the record.
const USA_OFFSET: usize = 0x30;
/// Marks the end of the attribute list.
const END_MARKER: u32 = 0xFFFF_FFFF;

/// Builder for one FILE record.
pub struct RecordBuilder {
    rec_no: u32,
    is_dir: bool,
    in_use: bool,
    base: u64,
    link_count: u16,
    /// Serialized attributes, written in order before the end marker.
    attrs: Vec<Vec<u8>>,
}

fn align8(n: usize) -> usize {
    (n + 7) & !7
}

impl RecordBuilder {
    /// A used, non-directory record with one hard link and no base record.
    pub fn new(rec_no: u32) -> Self {
        Self {
            rec_no,
            is_dir: false,
            in_use: true,
            base: 0,
            link_count: 1,
            attrs: Vec::new(),
        }
    }

    pub fn dir(mut self, is_dir: bool) -> Self {
        self.is_dir = is_dir;
        self
    }

    pub fn in_use(mut self, in_use: bool) -> Self {
        self.in_use = in_use;
        self
    }

    /// Raw 64-bit base-record reference (written as given, not masked).
    pub fn base(mut self, base: u64) -> Self {
        self.base = base;
        self
    }

    pub fn link_count(mut self, link_count: u16) -> Self {
        self.link_count = link_count;
        self
    }

    /// Appends a pre-serialized attribute (for malformed-input tests).
    pub fn raw_attr(mut self, attr: Vec<u8>) -> Self {
        self.attrs.push(attr);
        self
    }

    /// Appends `$STANDARD_INFORMATION` with the given mtime and DOS attributes.
    pub fn std_info(self, mtime: i64, attrs: u32) -> Self {
        self.raw_attr(std_info_attr(mtime, attrs))
    }

    /// Appends `$FILE_NAME` with a UTF-8 name (encoded to UTF-16).
    pub fn file_name(self, parent: u64, namespace: u8, name: &str) -> Self {
        let units: Vec<u16> = name.encode_utf16().collect();
        self.file_name_utf16(parent, namespace, &units)
    }

    /// Appends `$FILE_NAME` with raw UTF-16 units (e.g. a lone surrogate).
    pub fn file_name_utf16(self, parent: u64, namespace: u8, name: &[u16]) -> Self {
        self.raw_attr(file_name_attr(parent, namespace, name))
    }

    /// Appends an unnamed resident `$DATA` of `len` bytes.
    pub fn data_resident(self, len: u32) -> Self {
        self.raw_attr(data_resident_attr("", len))
    }

    /// Appends a named resident `$DATA` (an alternate data stream).
    pub fn data_resident_named(self, name: &str, len: u32) -> Self {
        self.raw_attr(data_resident_attr(name, len))
    }

    /// Appends an unnamed non-resident `$DATA` with the given encoded run list.
    pub fn data_nonresident(self, alloc: u64, real: u64, runs: &[u8]) -> Self {
        self.data_nonresident_with(NonResident::new(alloc, real, runs))
    }

    /// Appends a non-resident `$DATA` described by `spec`.
    pub fn data_nonresident_with(self, spec: NonResident) -> Self {
        self.raw_attr(data_nonresident_attr(&spec))
    }

    /// Builds the record and undoes the protection, as the scanner sees it
    /// after `apply_fixups` (1 KiB record, 512-byte sectors).
    pub fn build_plain(&self) -> Vec<u8> {
        let mut rec = self.build(1024, 512);
        bf_ntfs::apply_fixups(&mut rec, 512).expect("builder output has valid fixups");
        rec
    }

    /// Builds the protected record (sector tails replaced by the USN).
    pub fn build(&self, bytes_per_record: usize, bytes_per_sector: usize) -> Vec<u8> {
        assert!(bytes_per_sector >= 2 && bytes_per_record.is_multiple_of(bytes_per_sector));
        let sectors = bytes_per_record / bytes_per_sector;
        let usa_count = sectors + 1;
        let first_attr = align8(USA_OFFSET + 2 * usa_count);
        let attrs_len: usize = self.attrs.iter().map(Vec::len).sum();
        let used_size = align8(first_attr + attrs_len + 4);
        assert!(used_size <= bytes_per_record, "record too small");

        let mut rec = vec![0u8; bytes_per_record];
        rec[0..4].copy_from_slice(b"FILE");
        put_u16(&mut rec, 4, USA_OFFSET as u16);
        put_u16(&mut rec, 6, usa_count as u16);
        put_u16(&mut rec, 16, 1); // sequence number
        put_u16(&mut rec, 18, self.link_count);
        put_u16(&mut rec, 20, first_attr as u16);
        let flags = u16::from(self.in_use) | (u16::from(self.is_dir) << 1);
        put_u16(&mut rec, 22, flags);
        put_u32(&mut rec, 24, used_size as u32);
        put_u32(&mut rec, 28, bytes_per_record as u32);
        rec[32..40].copy_from_slice(&self.base.to_le_bytes());
        put_u16(&mut rec, 40, self.attrs.len() as u16); // next attribute id
        put_u32(&mut rec, 44, self.rec_no);

        let mut pos = first_attr;
        for attr in &self.attrs {
            rec[pos..pos + attr.len()].copy_from_slice(attr);
            pos += attr.len();
        }
        put_u32(&mut rec, pos, END_MARKER);

        protect(&mut rec, bytes_per_sector, usa_count);
        rec
    }
}

/// Description of a non-resident `$DATA` attribute.
pub struct NonResident {
    pub alloc: u64,
    pub real: u64,
    pub runs: Vec<u8>,
    pub name: String,
    /// Starting VCN of this piece of the stream (0 for the first piece).
    pub start_vcn: u64,
    /// `Some(size)`: set the compressed flag and write this compressed size.
    pub compressed: Option<u64>,
    /// `Some(size)`: set the sparse flag and write this compressed size.
    pub sparse: Option<u64>,
}

impl NonResident {
    /// An unnamed, uncompressed attribute starting at VCN 0.
    pub fn new(alloc: u64, real: u64, runs: &[u8]) -> Self {
        Self {
            alloc,
            real,
            runs: runs.to_vec(),
            name: String::new(),
            start_vcn: 0,
            compressed: None,
            sparse: None,
        }
    }

    pub fn start_vcn(mut self, vcn: u64) -> Self {
        self.start_vcn = vcn;
        self
    }

    pub fn compressed(mut self, compressed_size: u64) -> Self {
        self.compressed = Some(compressed_size);
        self
    }

    pub fn sparse(mut self, compressed_size: u64) -> Self {
        self.sparse = Some(compressed_size);
        self
    }

    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }
}

/// Resident attribute: 24-byte header, optional name, 8-aligned value.
fn resident_attr(type_code: u32, name: &str, value: &[u8]) -> Vec<u8> {
    let name_units: Vec<u16> = name.encode_utf16().collect();
    let value_off = align8(24 + 2 * name_units.len());
    let total = align8(value_off + value.len());
    let mut a = vec![0u8; total];
    put_u32(&mut a, 0, type_code);
    put_u32(&mut a, 4, total as u32);
    a[9] = name_units.len() as u8;
    put_u16(&mut a, 10, 24); // name offset
    put_u32(&mut a, 16, value.len() as u32);
    put_u16(&mut a, 20, value_off as u16);
    for (i, u) in name_units.iter().enumerate() {
        put_u16(&mut a, 24 + 2 * i, *u);
    }
    a[value_off..value_off + value.len()].copy_from_slice(value);
    a
}

fn std_info_attr(mtime: i64, attrs: u32) -> Vec<u8> {
    let mut v = vec![0u8; 72];
    v[8..16].copy_from_slice(&mtime.to_le_bytes());
    put_u32(&mut v, 32, attrs);
    resident_attr(0x10, "", &v)
}

fn file_name_attr(parent: u64, namespace: u8, name: &[u16]) -> Vec<u8> {
    let mut v = vec![0u8; 66 + 2 * name.len()];
    v[0..8].copy_from_slice(&parent.to_le_bytes());
    v[64] = name.len() as u8;
    v[65] = namespace;
    for (i, u) in name.iter().enumerate() {
        put_u16(&mut v, 66 + 2 * i, *u);
    }
    resident_attr(0x30, "", &v)
}

fn data_resident_attr(name: &str, len: u32) -> Vec<u8> {
    resident_attr(0x80, name, &vec![0xAA; len as usize])
}

/// Non-resident attribute: 64-byte header (72 with a compressed size), name,
/// then the run list.
fn data_nonresident_attr(spec: &NonResident) -> Vec<u8> {
    let compressed_size = spec.compressed.or(spec.sparse);
    let header_len = if compressed_size.is_some() { 72 } else { 64 };
    let name_units: Vec<u16> = spec.name.encode_utf16().collect();
    let runs_off = align8(header_len + 2 * name_units.len());
    let total = align8(runs_off + spec.runs.len());
    let mut a = vec![0u8; total];
    put_u32(&mut a, 0, 0x80);
    put_u32(&mut a, 4, total as u32);
    a[8] = 1;
    a[9] = name_units.len() as u8;
    put_u16(&mut a, 10, header_len as u16); // name offset
    let flags = u16::from(spec.compressed.is_some()) | (u16::from(spec.sparse.is_some()) << 15);
    put_u16(&mut a, 12, flags);
    a[16..24].copy_from_slice(&spec.start_vcn.to_le_bytes());
    put_u16(&mut a, 32, runs_off as u16);
    a[40..48].copy_from_slice(&spec.alloc.to_le_bytes());
    a[48..56].copy_from_slice(&spec.real.to_le_bytes());
    if let Some(size) = compressed_size {
        a[64..72].copy_from_slice(&size.to_le_bytes());
    }
    for (i, u) in name_units.iter().enumerate() {
        put_u16(&mut a, header_len + 2 * i, *u);
    }
    a[runs_off..runs_off + spec.runs.len()].copy_from_slice(&spec.runs);
    a
}

/// Saves each sector tail into the USA and writes the USN over it.
fn protect(rec: &mut [u8], bytes_per_sector: usize, usa_count: usize) {
    put_u16(rec, USA_OFFSET, USN);
    for i in 1..usa_count {
        let tail = i * bytes_per_sector - 2;
        let saved = [rec[tail], rec[tail + 1]];
        rec[USA_OFFSET + 2 * i..USA_OFFSET + 2 * i + 2].copy_from_slice(&saved);
        put_u16(rec, tail, USN);
    }
}

fn put_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
