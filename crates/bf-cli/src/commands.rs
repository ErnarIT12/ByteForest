//! The `drives`, `scan` and `bench` commands.

use std::io::{IsTerminal, Write};
use std::time::Duration;

use bf_core::{NodeView, SortKey, Tree};
use bf_scan::{
    CancelToken, ScanProgress, ScanRoot, ScanStats, VolumeSummary, is_elevated, list_volumes,
    scan_auto,
};
use serde_json::json;

use crate::cli::Command;
use crate::format::{human_size, top_table};
use crate::mem::peak_working_set;
use crate::target::parse_target;

/// Runs a command; the error string is printed as `error: <message>` by `main`.
pub fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Drives { json } => drives(json),
        Command::Scan { target, top, json } => scan(&target, top.clamp(1, 1000), json),
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
            format!("{}:", v.letter),
            v.label,
            v.fs,
            human_size(v.total),
            human_size(v.free)
        ));
    }
    out
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

fn scan(target: &str, top: usize, json: bool) -> Result<(), String> {
    let root = parse_target(target)?;
    let show_progress = !json && std::io::stderr().is_terminal();
    let (tree, stats) = scan_once(&root, show_progress)?;
    let rows = top_children(&tree, top);
    if json {
        return print_json(&json!({ "stats": stats, "top": rows }));
    }
    print!("{}", scan_header(&stats));
    println!();
    print!("{}", top_table(&rows, stats.bytes));
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
