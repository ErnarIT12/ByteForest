//! Human-readable sizes and the result table.

use bf_core::NodeView;

/// Formats a byte count with base-1024 units and one decimal (`1.5 KB`); bytes have none.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Like [`human_size`] with an explicit sign: `+4.8 GB`, `-120 MB`, `0 B`.
pub fn signed_size(delta: i64) -> String {
    match delta {
        0 => "0 B".to_string(),
        d if d > 0 => format!("+{}", human_size(d.unsigned_abs())),
        d => format!("-{}", human_size(d.unsigned_abs())),
    }
}

/// `YYYY-MM-DD HH:MM` in UTC for a unix time in milliseconds.
pub fn utc_minute(unix_ms: i64) -> String {
    let secs = unix_ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (proleptic Gregorian), after Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// Parses a byte size string with an optional base-1024 unit suffix (`B`, `K`/`KB`/`KiB`,
/// `M`/`MB`/`MiB`, `G`/`GB`/`GiB`, `T`/`TB`/`TiB`, ASCII case-insensitive).
pub fn parse_size(input: &str) -> Result<u64, String> {
    if input.is_empty() {
        return Err("size must not be empty".to_string());
    }
    let digit_end = input
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(input.len());
    if digit_end == 0 {
        return Err(format!(
            "invalid size `{input}`: expected a non-negative integer"
        ));
    }
    let (num_str, unit_str) = input.split_at(digit_end);
    let value: u64 = num_str
        .parse()
        .map_err(|_| format!("size `{input}` overflows 64-bit byte count"))?;
    let multiplier: u64 = match unit_str.to_ascii_uppercase().as_str() {
        "" | "B" => 1,
        "K" | "KB" | "KIB" => 1024,
        "M" | "MB" | "MIB" => 1024_u64.pow(2),
        "G" | "GB" | "GIB" => 1024_u64.pow(3),
        "T" | "TB" | "TIB" => 1024_u64.pow(4),
        _ => {
            return Err(format!(
                "invalid size unit `{unit_str}` in `{input}` (expected B, K/KB/KiB, M/MB/MiB, G/GB/GiB, or T/TB/TiB)"
            ));
        }
    };
    value
        .checked_mul(multiplier)
        .ok_or_else(|| format!("size `{input}` overflows 64-bit byte count"))
}

/// Encodes bytes as lowercase hexadecimal.
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Summary of a duplicate search across all confirmed groups.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DupesSummary {
    pub groups: u64,
    pub files: u64,
    pub reclaimable_bytes: u64,
    pub skipped: u64,
    pub bytes_read: u64,
    pub min_size: u64,
    pub candidates_files: u64,
    pub candidates_bytes: u64,
}

/// One confirmed duplicate group ready for text or JSON output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DupesGroupOutput {
    pub size: u64,
    pub reclaimable: u64,
    pub hash: String,
    pub files: Vec<String>,
}

/// Renders the one-line duplicate summary below the scan header.
pub fn dupes_summary_line(summary: &DupesSummary) -> String {
    let skipped = if summary.skipped > 0 {
        format!(", {} skipped", summary.skipped)
    } else {
        String::new()
    };
    format!(
        "duplicates: {} groups, {} files, {} reclaimable{skipped} (min size: {}, read: {})\n",
        summary.groups,
        summary.files,
        human_size(summary.reclaimable_bytes),
        human_size(summary.min_size),
        human_size(summary.bytes_read),
    )
}

/// Renders the duplicate groups list, or a clear message if no duplicates were found.
pub fn dupes_groups_text(groups: &[DupesGroupOutput], summary: &DupesSummary) -> String {
    if groups.is_empty() {
        return format!(
            "no duplicate files found (min size: {})\n",
            human_size(summary.min_size)
        );
    }
    let mut out = String::new();
    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let short_hash = &group.hash[..group.hash.len().min(12)];
        out.push_str(&format!(
            "{}. {} x {}  ({} reclaimable)  [{short_hash}]\n",
            i + 1,
            human_size(group.size),
            group.files.len(),
            human_size(group.reclaimable),
        ));
        for file in &group.files {
            out.push_str(&format!("   {file}\n"));
        }
    }
    if (groups.len() as u64) < summary.groups {
        out.push_str(&format!(
            "\nshowing {} of {} duplicate groups (raise --top to see more)\n",
            groups.len(),
            summary.groups,
        ));
    }
    out
}

/// Renders the top-children table: size, percent of root, files, name.
pub fn top_table(rows: &[NodeView], root_size: u64) -> String {
    let mut out = format!("{:>10}  {:>6}  {:>10}  NAME\n", "SIZE", "%", "FILES");
    for row in rows {
        let percent = if root_size == 0 {
            0.0
        } else {
            100.0 * row.size as f64 / root_size as f64
        };
        let suffix = if row.is_dir { "\\" } else { "" };
        out.push_str(&format!(
            "{:>10}  {:>5.1}%  {:>10}  {}{}\n",
            human_size(row.size),
            percent,
            row.files,
            row.name,
            suffix
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_sizes() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn signed_sizes_carry_an_explicit_sign() {
        assert_eq!(signed_size(0), "0 B");
        assert_eq!(signed_size(1536), "+1.5 KB");
        assert_eq!(signed_size(-125_829_120), "-120.0 MB");
        assert_eq!(signed_size(5_153_960_755), "+4.8 GB");
        assert!(signed_size(i64::MIN).starts_with('-'));
        assert!(signed_size(i64::MAX).starts_with('+'));
    }

    #[test]
    fn utc_minute_formats_known_instants() {
        assert_eq!(utc_minute(0), "1970-01-01 00:00");
        assert_eq!(utc_minute(1_700_000_000_000), "2023-11-14 22:13");
        assert_eq!(utc_minute(951_782_400_000), "2000-02-29 00:00");
        assert_eq!(utc_minute(-1000), "1969-12-31 23:59");
    }

    #[test]
    fn huge_sizes_stay_in_petabytes() {
        assert_eq!(human_size(1 << 50), "1.0 PB");
        assert!(human_size(u64::MAX).ends_with(" PB"));
    }

    #[test]
    fn parse_size_handles_bytes_and_base_1024_units() {
        assert_eq!(parse_size("0"), Ok(0));
        assert_eq!(parse_size("0M"), Ok(0));
        assert_eq!(parse_size("512"), Ok(512));
        assert_eq!(parse_size("512B"), Ok(512));
        assert_eq!(parse_size("512b"), Ok(512));
        assert_eq!(parse_size("10K"), Ok(10 * 1024));
        assert_eq!(parse_size("500K"), Ok(512_000));
        assert_eq!(parse_size("10kb"), Ok(10 * 1024));
        assert_eq!(parse_size("10KiB"), Ok(10 * 1024));
        assert_eq!(parse_size("1M"), Ok(bf_core::DEFAULT_DUPLICATE_MIN_SIZE));
        assert_eq!(parse_size("1m"), Ok(1_048_576));
        assert_eq!(parse_size("3MB"), Ok(3 * 1_048_576));
        assert_eq!(parse_size("3mib"), Ok(3 * 1_048_576));
        assert_eq!(parse_size("2GiB"), Ok(2 * (1 << 30)));
        assert_eq!(parse_size("2g"), Ok(2 * (1 << 30)));
        assert_eq!(parse_size("2GB"), Ok(2 * (1 << 30)));
        assert_eq!(parse_size("1T"), Ok(1 << 40));
        assert_eq!(parse_size("1tb"), Ok(1 << 40));
        assert_eq!(parse_size("1TiB"), Ok(1 << 40));
    }

    #[test]
    fn parse_size_rejects_whitespace_invalid_and_overflow() {
        for bad in [
            "",
            " ",
            " 1M",
            "1M ",
            "1 M",
            "-1",
            "-1M",
            "M",
            "1.5M",
            "10X",
            "10PB",
            "99999999999999999999999999",
            "16777216T",
        ] {
            assert!(parse_size(bad).is_err(), "expected error for `{bad}`");
        }
    }

    #[test]
    fn hex_encode_formats_lowercase_hex() {
        assert_eq!(hex_encode(&[]), "");
        assert_eq!(hex_encode(&[0x00, 0x0a, 0xff, 0x1b]), "000aff1b");
        assert_eq!(hex_encode(&[0xab; 32]).len(), 64);
    }

    #[test]
    fn dupes_text_formats_summary_groups_and_empty_state() {
        let empty = DupesSummary {
            groups: 0,
            files: 0,
            reclaimable_bytes: 0,
            skipped: 0,
            bytes_read: 0,
            min_size: 1_048_576,
            candidates_files: 0,
            candidates_bytes: 0,
        };
        let empty_summary = dupes_summary_line(&empty);
        assert!(empty_summary.contains("0 groups, 0 files, 0 B reclaimable"));
        assert!(!empty_summary.contains("skipped"));
        assert!(dupes_groups_text(&[], &empty).contains("no duplicate files found"));

        let summary = DupesSummary {
            groups: 2,
            files: 5,
            reclaimable_bytes: 3 * 1024,
            skipped: 1,
            bytes_read: 5 * 1024,
            min_size: 1024,
            candidates_files: 6,
            candidates_bytes: 6 * 1024,
        };
        let line = dupes_summary_line(&summary);
        assert!(line.contains("2 groups, 5 files, 3.0 KB reclaimable, 1 skipped"));

        let shown = vec![DupesGroupOutput {
            size: 2048,
            reclaimable: 2048,
            hash: "0123456789abcdef".repeat(4),
            files: vec!["/a/one.bin".to_string(), "/b/two.bin".to_string()],
        }];
        let body = dupes_groups_text(&shown, &summary);
        assert!(body.contains("1. 2.0 KB x 2  (2.0 KB reclaimable)  [0123456789ab]"));
        assert!(body.contains("   /a/one.bin\n"));
        assert!(body.contains("   /b/two.bin\n"));
        assert!(body.contains("showing 1 of 2 duplicate groups"));
    }
}
