//! Synthetic NTFS FILE-record generator for tests.
//!
//! Builds a record the way NTFS stores it on disk: header, update sequence
//! array (USA), attributes, end marker, and finally the "protection" step that
//! copies each sector's last two bytes into the USA and overwrites them with
//! the update sequence number.
//!
//! Task 4 supports only the header and the end marker. Attribute builders are
//! added later; they push serialized attributes into `attrs`.

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
