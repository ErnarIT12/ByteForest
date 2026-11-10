use crate::{NtfsError, u32_at, u64_at};

/// Bytes needed to read every field below (end of `MftStartLcn`).
const MIN_LEN: usize = 72;

/// Fields we need from `NTFS_VOLUME_DATA_BUFFER`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeInfo {
    pub serial: u64,
    pub bytes_per_sector: u32,
    pub bytes_per_cluster: u32,
    pub bytes_per_record: u32,
    pub mft_valid_data_length: u64,
    pub mft_start_lcn: u64,
    pub total_clusters: u64,
}

/// Reads an `i64` field that must be non-negative.
///
/// Windows declares these fields as signed `LARGE_INTEGER`, but a negative
/// value is meaningless, so it is reported as `Truncated` (corrupt buffer).
fn non_negative(buf: &[u8], off: usize) -> Result<u64, NtfsError> {
    let raw = u64_at(buf, off)?;
    i64::try_from(raw).map_err(|_| NtfsError::Truncated)?;
    Ok(raw)
}

/// Parses an `NTFS_VOLUME_DATA_BUFFER` (the output of `FSCTL_GET_NTFS_VOLUME_DATA`).
pub fn parse_volume_data(buf: &[u8]) -> Result<VolumeInfo, NtfsError> {
    if buf.len() < MIN_LEN {
        return Err(NtfsError::Truncated);
    }
    Ok(VolumeInfo {
        // The serial number is an opaque identifier: keep the raw 64 bits.
        serial: u64_at(buf, 0)?,
        total_clusters: non_negative(buf, 16)?,
        bytes_per_sector: u32_at(buf, 40)?,
        bytes_per_cluster: u32_at(buf, 44)?,
        bytes_per_record: u32_at(buf, 48)?,
        mft_valid_data_length: non_negative(buf, 56)?,
        mft_start_lcn: non_negative(buf, 64)?,
    })
}
