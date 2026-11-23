//! The `drives`, `scan` and `bench` commands.

use std::fs::File;
use std::io::{BufReader, BufWriter, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bf_core::{
    DiffStatus, NodeView, Snapshot, SnapshotDiff, SortKey, Tree, diff, read_snapshot,
    write_snapshot,
};
use bf_scan::{
    CancelToken, ScanProgress, ScanRoot, ScanStats, VolumeSummary, is_elevated, list_volumes,
    scan_auto,
};
use serde_json::json;

use crate::cli::Command;
use crate::format::{human_size, signed_size, top_table, utc_minute};
use crate::mem::peak_working_set;
use crate::target::parse_target;

/// Runs a command; the error string is printed as `error: <message>` by `main`.
pub fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Drives { json } => drives(json),
        Command::Scan {
            target,
            top,
            json,
            save_snapshot,
            diff,
        } => scan(&ScanArgs {
            target: &target,
            top: top.clamp(1, 1000),
            json,
            save_snapshot: save_snapshot.as_deref(),
            diff: diff.as_deref(),
        }),
        Command::Bench { target, runs, json } => bench(&target, runs, json),
    }
}

fn print_json(value: &serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string(value).map_err(|e| e.to_string())?;
    println!("{text}");
    Ok(())
}

fn drives(json: bool) -> Result<(), String> {
    let volumes = list_volumes();
    if json {
        return print_json(&serde_json::to_value(&volumes).map_err(|e| e.to_string())?);
    }
    print!("{}", drives_table(&volumes));
    Ok(())
}

fn drives_table(volumes: &[VolumeSummary]) -> String {
    let mut out = format!(
        "{:<6}  {:<20}  {:<8}  {:>10}  {:>10}\n",
        "DRIVE", "LABEL", "FS", "TOTAL", "FREE"
    );
    for v in volumes {
        out.push_str(&format!(
            "{:<6}  {:<20}  {:<8}  {:>10}  {:>10}\n",
            volume_target(v),
            v.label,
            v.fs,
            human_size(v.total),
            human_size(v.free)
        ));
    }
    out
}

fn volume_target(volume: &VolumeSummary) -> String {
    #[cfg(windows)]
    return format!("{}:", volume.letter);
    #[cfg(target_os = "macos")]
    return volume.path.clone();
}

/// Scans once, drawing a one-line progress display on stderr when `show_progress` is set.
fn scan_once(root: &ScanRoot, show_progress: bool) -> Result<(Tree, ScanStats), String> {
    let progress = move |p: ScanProgress| {
        if show_progress {
            let mut err = std::io::stderr();
            let _ = write!(
                err,
                "\r{:?}: {} files, {} dirs, {}   ",
                p.phase,
                p.files,
                p.dirs,
                human_size(p.bytes)
            );
            let _ = err.flush();
        }
    };
    let result = scan_auto(root, &progress, &CancelToken::new());
    if show_progress {
        eprint!("\r{:60}\r", "");
    }
    result.map_err(|e| e.to_string())
}

fn top_children(tree: &Tree, n: usize) -> Vec<NodeView> {
    tree.children(tree.root(), SortKey::SizeDesc, 0, n)
}

struct ScanArgs<'a> {
    target: &'a str,
    top: usize,
    json: bool,
    save_snapshot: Option<&'a Path>,
    diff: Option<&'a Path>,
}

/// Whether file names compare case-insensitively on this platform's usual file systems.
fn case_insensitive_names() -> bool {
    cfg!(windows) || cfg!(target_os = "macos")
}

fn load_snapshot(path: &Path) -> Result<Snapshot, String> {
    let file =
        File::open(path).map_err(|e| format!("cannot open snapshot {}: {e}", path.display()))?;
    read_snapshot(BufReader::new(file))
        .map_err(|e| format!("cannot read snapshot {}: {e}", path.display()))
}

/// Writes to `<file>.tmp` and renames it over `path`, so a failure never leaves a partial file.
fn save_snapshot_file(tree: &Tree, label: &str, path: &Path) -> Result<(), String> {
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    let write = || -> Result<(), String> {
        let file = File::create(&tmp).map_err(|e| e.to_string())?;
        let mut w = BufWriter::new(file);
        write_snapshot(tree, label, created, &mut w).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        drop(w);
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("cannot save snapshot {}: {e}", path.display())
    })
}

fn status_name(status: DiffStatus) -> &'static str {
    match status {
        DiffStatus::Added => "added",
        DiffStatus::Grown => "grown",
        DiffStatus::Shrunk => "shrunk",
        DiffStatus::Unchanged => "unchanged",
    }
}

fn diff_json(snap: &Snapshot, d: &SnapshotDiff, tree: &Tree, limit: usize) -> serde_json::Value {
    let summary = d.summary();
    let changes: Vec<_> = d
        .top_changes(tree, limit)
        .into_iter()
        .map(|(id, e)| {
            json!({
                "path": tree.path_of(id),
                "status": status_name(e.status),
                "base": e.base,
                "current": e.current,
                "delta": e.delta,
            })
        })
        .collect();
    json!({
        "base_label": snap.label,
        "base_created_unix_ms": snap.created_unix_ms,
        "total_delta": summary.total_delta,
        "added": { "bytes": summary.added.bytes, "count": summary.added.count },
        "removed": { "bytes": summary.removed.bytes, "count": summary.removed.count },
        "changes": changes,
    })
}

fn diff_text(snap: &Snapshot, d: &SnapshotDiff, tree: &Tree, limit: usize) -> String {
    let summary = d.summary();
    let mut out = format!(
        "changes since snapshot of {} ({} UTC): net {}, {} new ({}), {} gone ({})
",
        snap.label,
        utc_minute(snap.created_unix_ms),
        signed_size(summary.total_delta),
        summary.added.count,
        human_size(summary.added.bytes),
        summary.removed.count,
        human_size(summary.removed.bytes),
    );
    out.push_str(&format!(
        "{:>10}  {:>10}  PATH
",
        "CHANGE", "NOW"
    ));
    for (id, e) in d.top_changes(tree, limit) {
        let change = if e.status == DiffStatus::Added {
            "new".to_string()
        } else {
            signed_size(e.delta)
        };
        out.push_str(&format!(
            "{:>10}  {:>10}  {}
",
            change,
            human_size(e.current),
            tree.path_of(id)
        ));
    }
    out
}

fn scan(args: &ScanArgs) -> Result<(), String> {
    let root = parse_target(args.target)?;
    // Load the baseline first so a bad file fails before a long scan.
    let baseline = args.diff.map(load_snapshot).transpose()?;
    let show_progress = !args.json && std::io::stderr().is_terminal();
    let (tree, stats) = scan_once(&root, show_progress)?;
    let rows = top_children(&tree, args.top);
    let diffed = baseline
        .as_ref()
        .map(|snap| (snap, diff(&snap.tree, &tree, case_insensitive_names())));

    if args.json {
        let mut value = json!({ "stats": stats, "top": rows });
        if let Some((snap, d)) = &diffed {
            value["diff"] = diff_json(snap, d, &tree, args.top);
        }
        print_json(&value)?;
    } else {
        print!("{}", scan_header(&stats));
        println!();
        print!("{}", top_table(&rows, stats.bytes));
        if let Some((snap, d)) = &diffed {
            println!();
            print!("{}", diff_text(snap, d, &tree, args.top));
        }
    }
    if let Some(path) = args.save_snapshot {
        save_snapshot_file(&tree, args.target, path)?;
        if !args.json {
            println!("snapshot saved to {}", path.display());
        }
    }
    Ok(())
}

fn scan_header(stats: &ScanStats) -> String {
    let mut out = format!(
        "method: {:?}  files: {}  dirs: {}  size: {}  elapsed: {:.2} s  skipped_records: {}  errors: {}\n",
        stats.method,
        stats.files,
        stats.dirs,
        human_size(stats.bytes),
        Duration::from_millis(stats.elapsed_ms).as_secs_f64(),
        stats.skipped_records,
        stats.errors
    );
    if let Some(reason) = &stats.fallback_reason {
        out.push_str(&format!("note: {reason}\n"));
    }
    out
}

/// Median of a non-empty list (mean of the two middle values for an even count).
fn median(values: &[u64]) -> u64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / 2
    }
}

fn bench(target: &str, runs: u32, json: bool) -> Result<(), String> {
    let root = parse_target(target)?;
    let mut results: Vec<ScanStats> = Vec::new();
    let mut nodes = 0usize;
    for _ in 0..runs {
        let (tree, stats) = scan_once(&root, false)?;
        nodes = tree.len();
        results.push(stats);
        // `tree` drops here, so each run starts from a freed tree; peak memory is the max.
    }
    let elapsed: Vec<u64> = results.iter().map(|s| s.elapsed_ms).collect();
    let median_ms = median(&elapsed);
    let peak = peak_working_set().unwrap_or(0);
    let bytes_per_node = if nodes == 0 {
        0.0
    } else {
        peak as f64 / nodes as f64
    };
    let elevated = is_elevated();

    if json {
        let runs_json: Vec<_> = results
            .iter()
            .map(|s| json!({ "elapsed_ms": s.elapsed_ms, "method": s.method }))
            .collect();
        return print_json(&json!({
            "target": target,
            "elevated": elevated,
            "runs": runs_json,
            "median_ms": median_ms,
            "nodes": nodes,
            "peak_working_set": peak,
            "bytes_per_node": bytes_per_node,
        }));
    }

    println!(
        "target: {target}  (elevated: {})",
        if elevated { "yes" } else { "no" }
    );
    for (i, s) in results.iter().enumerate() {
        println!("run {}: {} ms  ({:?})", i + 1, s.elapsed_ms, s.method);
    }
    if let Some(reason) = results.first().and_then(|s| s.fallback_reason.as_ref()) {
        println!("note: {reason}");
    }
    println!("median: {median_ms} ms");
    println!("nodes: {nodes}");
    println!("peak working set: {}", human_size(peak));
    println!("bytes per node: {bytes_per_node:.1}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_odd_and_even() {
        assert_eq!(median(&[5, 1, 3]), 3);
        assert_eq!(median(&[4, 2]), 3);
        assert_eq!(median(&[7]), 7);
    }
}
