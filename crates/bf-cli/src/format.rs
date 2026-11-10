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
    fn huge_sizes_stay_in_petabytes() {
        assert_eq!(human_size(1 << 50), "1.0 PB");
        assert!(human_size(u64::MAX).ends_with(" PB"));
    }
}
