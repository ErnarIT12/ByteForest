//! Elevated diagnostic for the MFT scanner.
//!
//! ```text
//! cargo run -p bf-scan --release --example mft_probe -- C
//! ```
//!
//! Prints each step of the MFT path (volume data, `$MFT` extents, block reader, read plan,
//! test reads), then runs two full scans and prints their stats or whole error chains:
//! the normal `MftScanner` scan (raw blocks, or record by record if blocks fail) and a scan
//! forced to read record by record with `FSCTL_GET_NTFS_FILE_RECORD`. Exits with 0 when both
//! scans succeed and 1 otherwise.

use std::error::Error;
use std::process::ExitCode;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

use bf_core::Tree;
use bf_scan::{
    CancelToken, MftScanner, ScanError, ScanProgress, ScanRoot, ScanStats, Scanner, probe_mft,
    probe_mft_records,
};

fn print_chain(err: &dyn Error) {
    println!("error: {err}");
    println!("debug: {err:?}");
    let mut source = err.source();
    while let Some(cause) = source {
        println!("caused by: {cause}");
        source = cause.source();
    }
}

/// Prints the outcome of one scan; returns whether it succeeded.
fn report(result: Result<(Tree, ScanStats), ScanError>, started: Instant) -> bool {
    match result {
        Ok((tree, stats)) => {
            let total = tree.total(tree.root());
            println!("{stats:#?}");
            println!(
                "root totals: files {} dirs {} size {} alloc {} (wall {} ms)",
                total.files,
                total.dirs,
                total.size,
                total.alloc,
                started.elapsed().as_millis()
            );
            true
        }
        Err(err) => {
            print_chain(&err);
            false
        }
    }
}

fn main() -> ExitCode {
    let letter = std::env::args()
        .nth(1)
        .and_then(|arg| arg.chars().next())
        .unwrap_or('C');

    println!("== probe ==");
    if let Err(err) = probe_mft(letter, &mut |line| println!("{line}")) {
        print_chain(&err);
    }

    let last_phase = Mutex::new(None);
    let progress = |p: ScanProgress| {
        // Print phase changes only, to keep the output short.
        let mut last = last_phase.lock().unwrap_or_else(PoisonError::into_inner);
        if *last != Some(p.phase) {
            println!(
                "phase {:?}: files {} dirs {} bytes {}",
                p.phase, p.files, p.dirs, p.bytes
            );
            *last = Some(p.phase);
        }
    };

    println!("== full MftScanner scan ==");
    let started = Instant::now();
    let auto = MftScanner.scan(&ScanRoot::Volume(letter), &progress, &CancelToken::new());
    let auto_ok = report(auto, started);

    println!("== record-by-record scan (FSCTL_GET_NTFS_FILE_RECORD) ==");
    *last_phase.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let started = Instant::now();
    let records = probe_mft_records(letter, &progress, &CancelToken::new());
    let records_ok = report(records, started);

    if auto_ok && records_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
