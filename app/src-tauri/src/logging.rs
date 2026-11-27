//! Daily rolling log file in `%LOCALAPPDATA%\ByteForest\logs`, with stderr as a fallback.

use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};

/// Sets up the global subscriber. Keep the returned guard alive until the app exits so
/// buffered log lines are flushed.
pub fn init() -> Option<WorkerGuard> {
    if let Some(dir) = log_dir()
        && std::fs::create_dir_all(&dir).is_ok()
        && let Ok(appender) = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("byteforest.log")
            .build(&dir)
    {
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let _ = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_ansi(false)
            .try_init();
        return Some(guard);
    }
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
    None
}

fn log_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = PathBuf::from(std::env::var_os("LOCALAPPDATA")?);
    #[cfg(target_os = "macos")]
    let base = std::env::home_dir()?.join("Library/Logs");
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::home_dir()?.join(".local/share");
    Some(base.join("ByteForest").join("logs"))
}
