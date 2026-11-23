//! Flat reports of a subtree for spreadsheets and scripts (CSV and JSON).

use std::io::{self, Write};

use serde::Serialize;

use crate::flags::NodeFlags;
use crate::tree::{NONE, NodeId, Tree};
use crate::view::SortKey;

/// Deepest level a report may reach below its root.
pub const MAX_REPORT_DEPTH: u8 = 16;

/// Most rows a report may hold.
pub const MAX_REPORT_ROWS: usize = 1_000_000;

/// How much of the tree a report covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReportOptions {
    /// Levels below the report root to include (`0` is the root row only).
    pub max_depth: u8,
    /// Row limit, root row included.
    pub max_rows: usize,
    /// Separator between the parts of [`ReportRow::relative_path`].
    pub path_separator: char,
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_rows: 10_000,
            path_separator: '/',
        }
    }
}

impl ReportOptions {
    /// The options with `max_depth` and `max_rows` forced into their allowed ranges.
    pub fn clamped(self) -> Self {
        Self {
            max_depth: self.max_depth.min(MAX_REPORT_DEPTH),
            max_rows: self.max_rows.clamp(1, MAX_REPORT_ROWS),
            path_separator: self.path_separator,
        }
    }
}

/// One node of a report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReportRow {
    /// Levels below the report root (`0` for the root itself).
    pub depth: u8,
    /// Path below the report root, parts joined with the requested separator; empty for the
    /// root row.
    pub relative_path: String,
    pub name: String,
    pub is_dir: bool,
    /// Logical size of the subtree in bytes.
    pub size: u64,
    /// Allocated size of the subtree in bytes.
    pub alloc: u64,
    /// Files in the subtree (`1` for a file).
    pub files: u64,
    /// Directories below the node.
    pub dirs: u64,
    /// Modification time as UTC `YYYY-MM-DDTHH:MM:SSZ`, `None` when unknown.
    pub modified: Option<String>,
    /// Size divided by the size of the report root, `0.0` when that is empty.
    pub share_of_root: f64,
}

/// Rows of a subtree, biggest first within each folder, parents before their children.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub rows: Vec<ReportRow>,
    /// More rows existed than `max_rows` allowed.
    pub truncated: bool,
}

impl Tree {
    /// Builds a report of `id` and its descendants down to `opts.max_depth` (clamped), at most
    /// `opts.max_rows` rows. `None` when `id` is out of range or was removed.
    pub fn report(&self, id: NodeId, opts: ReportOptions) -> Option<Report> {
        if id.idx() >= self.len() || self.is_removed(id) {
            return None;
        }
        let opts = opts.clamped();
        let mut out = Report {
            rows: Vec::new(),
            truncated: false,
        };
        let root_size = self.total(id).size;
        let mut path = Vec::new();
        self.report_node(id, 0, &mut path, root_size, &opts, &mut out);
        Some(out)
    }

    /// Adds `id` and its subtree; returns `false` once the row limit stopped the report.
    fn report_node(
        &self,
        id: NodeId,
        depth: u8,
        path: &mut Vec<String>,
        root_size: u64,
        opts: &ReportOptions,
        out: &mut Report,
    ) -> bool {
        if out.rows.len() >= opts.max_rows {
            out.truncated = true;
            return false;
        }
        let total = self.total(id);
        let is_dir = self.dir_index[id.idx()] != NONE;
        out.rows.push(ReportRow {
            depth,
            relative_path: path.join(&opts.path_separator.to_string()),
            name: self.name(id).to_string(),
            is_dir,
            size: total.size,
            alloc: total.alloc,
            files: total.files,
            dirs: total.dirs,
            modified: filetime_to_utc(self.mtime(id)),
            share_of_root: if root_size == 0 {
                0.0
            } else {
                total.size as f64 / root_size as f64
            },
        });
        if !is_dir || depth >= opts.max_depth {
            return true;
        }
        let children = self.child_order(id, SortKey::SizeDesc);
        for &c in children {
            let child = NodeId(c);
            if self.flags[child.idx()].contains(NodeFlags::REMOVED) {
                continue;
            }
            path.push(self.name(child).to_string());
            let keep_going = self.report_node(child, depth + 1, path, root_size, opts, out);
            path.pop();
            if !keep_going {
                return false;
            }
        }
        true
    }
}

/// Windows FILETIME to `YYYY-MM-DDTHH:MM:SSZ`; `None` for zero, negative or absurd values.
fn filetime_to_utc(filetime: i64) -> Option<String> {
    const UNIX_EPOCH_AS_FILETIME_SECS: i64 = 11_644_473_600;
    if filetime <= 0 {
        return None;
    }
    let secs = filetime / 10_000_000 - UNIX_EPOCH_AS_FILETIME_SECS;
    // Keep to years 1970..=9999 so the output stays four digits.
    if !(0..253_402_300_800).contains(&secs) {
        return None;
    }
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
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}

/// Column titles of the CSV output.
const CSV_HEADER: [&str; 10] = [
    "depth",
    "relative_path",
    "name",
    "type",
    "size_bytes",
    "allocated_bytes",
    "files",
    "folders",
    "modified_utc",
    "share_of_root",
];

/// Makes a text cell safe against spreadsheet formula injection: a cell that starts with
/// `=`, `+`, `-`, `@`, a tab or a carriage return gets a leading single quote.
fn defuse_formula(text: &str) -> String {
    if text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_string()
    }
}

/// RFC 4180 quoting: wraps the cell in quotes (doubling inner quotes) when it contains the
/// delimiter, a quote or a line break.
fn quote_cell(cell: &str, delimiter: char) -> String {
    if cell.contains([delimiter, '"', '\r', '\n']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

/// Writes the report as CSV: UTF-8 with a byte order mark (so Excel detects the encoding), a
/// header row, `\r\n` line ends, RFC 4180 quoting and formula-injection protection on the path
/// and name columns.
///
/// `delimiter` is usually `,` (or `;`/tab); quotes and line breaks are rejected.
pub fn write_csv<W: Write>(report: &Report, delimiter: char, w: &mut W) -> io::Result<()> {
    if matches!(delimiter, '"' | '\r' | '\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid CSV delimiter",
        ));
    }
    let d = delimiter.to_string();
    w.write_all("\u{FEFF}".as_bytes())?;
    write!(w, "{}\r\n", CSV_HEADER.join(&d))?;
    for row in &report.rows {
        let cells = [
            row.depth.to_string(),
            quote_cell(&defuse_formula(&row.relative_path), delimiter),
            quote_cell(&defuse_formula(&row.name), delimiter),
            (if row.is_dir { "folder" } else { "file" }).to_string(),
            row.size.to_string(),
            row.alloc.to_string(),
            row.files.to_string(),
            row.dirs.to_string(),
            row.modified.clone().unwrap_or_default(),
            format!("{:.6}", row.share_of_root),
        ];
        write!(w, "{}\r\n", cells.join(&d))?;
    }
    Ok(())
}
