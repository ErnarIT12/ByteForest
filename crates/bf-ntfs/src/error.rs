/// Errors from parsing NTFS structures.
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum NtfsError {
    /// The buffer is too short, or a size/offset field points outside it.
    #[error("buffer too short")]
    Truncated,
    /// The record does not start with `FILE`.
    #[error("bad magic")]
    BadMagic,
    /// A sector tail does not match the update sequence number.
    #[error("fixup mismatch")]
    FixupMismatch,
    /// An attribute at the given record offset is malformed.
    #[error("malformed attribute at {0}")]
    BadAttribute(usize),
}
