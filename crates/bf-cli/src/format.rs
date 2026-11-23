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
}
