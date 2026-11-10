//! Elevated diagnostic for the MFT scanner.
//!
//! ```text
//! cargo run -p bf-scan --release --example mft_probe -- C
//! ```
//!
//! Prints each step of the MFT path (volume data, `$MFT` extents, read plan, test reads), then
//! runs a full `MftScanner` scan and prints its stats or the whole error chain. Exits with 0
//! when the scan succeeds and 1 otherwise.

use std::error::Error;
use std::process::ExitCode;
use std::sync::{Mutex, PoisonError};

use bf_scan::{CancelToken, MftScanner, ScanProgress, ScanRoot, Scanner, probe_mft};

fn print_chain(err: &dyn Error) {
    println!("error: {err}");
    println!("debug: {err:?}");
    let mut source = err.source();
    while let Some(cause) = source {
        println!("caused by: {cause}");
        source = cause.source();
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

    println!("== full MftScanner scan ==");
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
    let result = MftScanner.scan(&ScanRoot::Volume(letter), &progress, &CancelToken::new());
    match result {
        Ok((tree, stats)) => {
            let total = tree.total(tree.root());
            println!("{stats:#?}");
            println!(
                "root totals: files {} dirs {} size {} alloc {}",
                total.files, total.dirs, total.size, total.alloc
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            print_chain(&err);
            ExitCode::FAILURE
        }
    }
}
