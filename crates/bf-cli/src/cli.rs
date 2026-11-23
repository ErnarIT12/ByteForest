//! Command-line arguments.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// ByteForest disk-space analyzer (command-line tool).
#[derive(Parser, Debug)]
#[command(name = "byteforest", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// The available commands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// List mounted drives.
    Drives {
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Scan a drive or folder and print the largest children of its root.
    Scan {
        /// Drive (`C:`) or folder path.
        target: String,
        /// How many children to show (clamped to 1..=1000).
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// Print one JSON object instead of text.
        #[arg(long)]
        json: bool,
        /// Save the scanned tree as a snapshot file (written after any --diff comparison).
        #[arg(long, value_name = "FILE")]
        save_snapshot: Option<PathBuf>,
        /// Compare the scan with a snapshot file and list what grew or is new.
        #[arg(long, value_name = "FILE")]
        diff: Option<PathBuf>,
        /// Leave out a name (`node_modules`, `*.tmp`) or an absolute path below the target
        /// (`C:\Windows\WinSxS`). Repeatable.
        #[arg(long, value_name = "PATTERN")]
        exclude: Vec<String>,
        /// Write a report of the scan to a file; `.json` gives JSON, anything else CSV.
        #[arg(long, value_name = "FILE")]
        export: Option<PathBuf>,
        /// Folder levels below the target that --export includes.
        #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=16))]
        export_depth: u8,
        /// Most rows --export writes (1 to 1000000).
        #[arg(long, default_value_t = 10_000, value_parser = clap::value_parser!(u32).range(1..=1_000_000))]
        export_rows: u32,
    },
    /// Scan several times and report timing and memory use.
    Bench {
        /// Drive (`C:`) or folder path.
        target: String,
        /// Number of scans.
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=100))]
        runs: u32,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}
