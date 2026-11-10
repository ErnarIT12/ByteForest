use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

use crate::ProgressFn;

/// Minimum time between two throttled progress callbacks (10 calls per second).
const MIN_INTERVAL_MS: u64 = 100;

/// Stage of a scan, shown by the UI next to the counters.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// Reading raw data from the volume (MFT scanner).
    #[default]
    Reading,
    /// Parsing the raw data into records (MFT scanner).
    Parsing,
    /// Walking directories (walk scanner).
    Walking,
    /// Linking records into the final tree.
    Building,
}

/// Counters reported while a scan runs.
#[derive(Serialize, Clone, Copy, Debug, Default)]
pub struct ScanProgress {
    /// Files seen so far.
    pub files: u64,
    /// Directories seen so far.
    pub dirs: u64,
    /// Total logical size of the files seen so far.
    pub bytes: u64,
    /// Current stage.
    pub phase: Phase,
}

/// Shared counters plus a time gate, so many worker threads can report progress cheaply.
///
/// [`ProgressReporter::tick`] calls back at most 10 times per second; [`ProgressReporter::finish`]
/// always calls back once.
pub struct ProgressReporter<'a> {
    callback: ProgressFn<'a>,
    start: Instant,
    /// Milliseconds since `start` of the last callback; `u64::MAX` before the first one.
    last_emit_ms: AtomicU64,
    files: AtomicU64,
    dirs: AtomicU64,
    bytes: AtomicU64,
}

impl<'a> ProgressReporter<'a> {
    /// Creates a reporter with zeroed counters.
    pub fn new(callback: ProgressFn<'a>) -> Self {
        Self {
            callback,
            start: Instant::now(),
            last_emit_ms: AtomicU64::new(u64::MAX),
            files: AtomicU64::new(0),
            dirs: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
        }
    }

    /// Adds to the counters and calls back if enough time has passed since the last call.
    pub fn tick(&self, files: u64, dirs: u64, bytes: u64, phase: Phase) {
        self.files.fetch_add(files, Ordering::Relaxed);
        self.dirs.fetch_add(dirs, Ordering::Relaxed);
        self.bytes.fetch_add(bytes, Ordering::Relaxed);

        let now_ms = self.start.elapsed().as_millis() as u64;
        let last = self.last_emit_ms.load(Ordering::Relaxed);
        let due = last == u64::MAX || now_ms.saturating_sub(last) >= MIN_INTERVAL_MS;
        // Only the thread that wins the exchange reports; the others skip this round.
        if due
            && self
                .last_emit_ms
                .compare_exchange(last, now_ms, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            (self.callback)(self.snapshot(phase));
        }
    }

    /// Calls back once with the current totals, ignoring the time gate.
    pub fn finish(&self, phase: Phase) {
        (self.callback)(self.snapshot(phase));
    }

    fn snapshot(&self, phase: Phase) -> ScanProgress {
        ScanProgress {
            files: self.files.load(Ordering::Relaxed),
            dirs: self.dirs.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            phase,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    #[test]
    fn tick_is_throttled_and_finish_always_reports() {
        let calls = AtomicUsize::new(0);
        let cb = |_: ScanProgress| {
            calls.fetch_add(1, Ordering::Relaxed);
        };
        let reporter = ProgressReporter::new(&cb);
        for _ in 0..1000 {
            reporter.tick(1, 0, 10, Phase::Walking);
        }
        // The first tick reports; the rest fall inside the 100 ms window (the loop is fast).
        let after_ticks = calls.load(Ordering::Relaxed);
        assert!((1..=3).contains(&after_ticks), "got {after_ticks}");

        reporter.finish(Phase::Building);
        assert_eq!(calls.load(Ordering::Relaxed), after_ticks + 1);
    }

    #[test]
    fn finish_reports_totals() {
        let last = std::sync::Mutex::new(ScanProgress::default());
        let cb = |p: ScanProgress| *last.lock().unwrap() = p;
        let reporter = ProgressReporter::new(&cb);
        reporter.tick(2, 1, 30, Phase::Walking);
        reporter.tick(1, 0, 5, Phase::Walking);
        reporter.finish(Phase::Building);
        let p = *last.lock().unwrap();
        assert_eq!((p.files, p.dirs, p.bytes), (3, 1, 35));
        assert_eq!(p.phase, Phase::Building);
    }
}
