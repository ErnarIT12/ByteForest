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
