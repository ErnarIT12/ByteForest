//! Lightweight on-demand system resource overview (CPU, memory, top processes).
//!
//! Keeps a single reusable [`sysinfo::System`] in app state and only refreshes when the
//! front end explicitly invokes `get_system_overview` (zero background polling/CPU usage).

use std::cmp::Ordering;
use std::sync::Arc;

use serde::Serialize;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::State;

use crate::state::AppState;

/// Maximum number of memory-heaviest processes returned by [`get_system_overview`].
const TOP_PROCESS_LIMIT: usize = 8;

/// One running process summarized for the Overview screen.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ProcessInfo {
    pub name: String,
    pub pid: u32,
    pub memory_bytes: u64,
    pub cpu_pct: f32,
}

/// Snapshot of current CPU, memory, and top processes by memory.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SystemOverview {
    pub cpu_usage_pct: f32,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub top_processes: Vec<ProcessInfo>,
}

/// Reusable `sysinfo::System` wrapper that avoids reallocating OS handles on every poll.
#[derive(Default)]
pub struct SystemOverviewSampler {
    sys: Option<System>,
    cpu_metadata_loaded: bool,
}

impl SystemOverviewSampler {
    /// Refreshes CPU, RAM, and process memory/CPU usage on demand and returns a summary.
    pub fn sample(&mut self) -> Result<SystemOverview, String> {
        let sys = match self.sys.as_mut() {
            Some(existing) => existing,
            None => {
                let created = System::new()
                    .map_err(|e| format!("failed to initialize system monitor: {e}"))?;
                self.sys.insert(created)
            }
        };

        if !self.cpu_metadata_loaded {
            sys.refresh_cpu_all();
            self.cpu_metadata_loaded = true;
        } else {
            sys.refresh_cpu_specifics(CpuRefreshKind::nothing().with_usage());
        }
        sys.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );

        let cpus = sys.cpus();
        let cpu_cores = cpus.len();
        let cpu_brand = cpus
            .first()
            .map(|c| normalize_brand(c.brand()))
            .unwrap_or_default();
        let cpu_usage_pct = sanitize_pct(sys.global_cpu_usage());

        let mem_total_bytes = sys.total_memory();
        let mem_used_bytes = sys.used_memory().min(mem_total_bytes);

        let raw_processes: Vec<ProcessInfo> = sys
            .processes()
            .iter()
            .map(|(pid, proc_info)| {
                let pid_u32 = pid.as_u32();
                ProcessInfo {
                    name: format_process_name(&proc_info.name().to_string_lossy(), pid_u32),
                    pid: pid_u32,
                    memory_bytes: proc_info.memory(),
                    cpu_pct: normalize_process_cpu(proc_info.cpu_usage(), cpu_cores),
                }
            })
            .collect();

        Ok(SystemOverview {
            cpu_usage_pct,
            cpu_brand,
            cpu_cores,
            mem_used_bytes,
            mem_total_bytes,
            top_processes: select_top_processes(raw_processes, TOP_PROCESS_LIMIT),
        })
    }
}

/// Collapses repeated whitespace in CPU brand strings reported by firmware/OS APIs.
fn normalize_brand(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Ensures every process row has a clean, non-empty display name.
fn format_process_name(raw: &str, pid: u32) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        format!("pid {pid}")
    } else {
        trimmed.to_string()
    }
}

/// Clamps an arbitrary percentage into `[0.0, 100.0]` rounded to one decimal place.
fn sanitize_pct(raw: f32) -> f32 {
    if !raw.is_finite() {
        return 0.0;
    }
    (raw.clamp(0.0, 100.0) * 10.0).round() / 10.0
}

/// Normalizes multi-core per-process CPU usage onto `[0.0, 100.0]` of total CPU capacity.
fn normalize_process_cpu(raw_cpu: f32, cpu_cores: usize) -> f32 {
    if !raw_cpu.is_finite() {
        return 0.0;
    }
    let divisor = cpu_cores.max(1) as f32;
    sanitize_pct(raw_cpu / divisor)
}

/// Sorts processes deterministically by memory desc, CPU desc, name asc, PID asc, capped to `limit`.
fn select_top_processes(mut items: Vec<ProcessInfo>, limit: usize) -> Vec<ProcessInfo> {
    items.sort_by(|a, b| {
        b.memory_bytes
            .cmp(&a.memory_bytes)
            .then_with(|| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap_or(Ordering::Equal))
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.pid.cmp(&b.pid))
    });
    items.truncate(limit);
    items
}

/// Samples current CPU, memory, and top processes on a blocking worker thread.
#[tauri::command]
pub async fn get_system_overview(state: State<'_, AppState>) -> Result<SystemOverview, String> {
    let sampler = Arc::clone(&state.system);
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = sampler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.sample()
    })
    .await
    .map_err(|e| format!("failed to collect system overview: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc_entry(name: &str, pid: u32, memory_bytes: u64, cpu_pct: f32) -> ProcessInfo {
        ProcessInfo {
            name: name.to_string(),
            pid,
            memory_bytes,
            cpu_pct,
        }
    }

    #[test]
    fn sorts_top_processes_by_memory_then_cpu_name_pid_and_truncates() {
        let items = vec![
            proc_entry("zeta", 30, 100, 1.0),
            proc_entry("alpha", 12, 500, 2.0),
            proc_entry("beta", 11, 500, 2.0),
            proc_entry("gamma", 10, 500, 8.5),
            proc_entry("alpha", 5, 500, 2.0),
            proc_entry("big", 1, 900, 0.5),
            proc_entry("mid", 2, 300, 4.0),
            proc_entry("small-a", 3, 50, 0.0),
            proc_entry("small-b", 4, 25, 0.0),
            proc_entry("huge", 99, 2_000, 12.0),
        ];

        let top = select_top_processes(items, 8);
        assert_eq!(top.len(), 8);
        let summary: Vec<(&str, u32, u64)> = top
            .iter()
            .map(|p| (p.name.as_str(), p.pid, p.memory_bytes))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("huge", 99, 2_000),
                ("big", 1, 900),
                ("gamma", 10, 500),
                ("alpha", 5, 500),
                ("alpha", 12, 500),
                ("beta", 11, 500),
                ("mid", 2, 300),
                ("zeta", 30, 100),
            ]
        );
    }

    #[test]
    fn formats_brand_and_process_names_cleanly() {
        assert_eq!(
            normalize_brand("  Intel(R)  Core(TM)   i7-13700K  "),
            "Intel(R) Core(TM) i7-13700K"
        );
        assert_eq!(normalize_brand("   "), "");
        assert_eq!(format_process_name("  Code Helper  ", 42), "Code Helper");
        assert_eq!(format_process_name("   ", 42), "pid 42");
    }

    #[test]
    fn sanitizes_and_normalizes_cpu_percentages() {
        assert_eq!(sanitize_pct(f32::NAN), 0.0);
        assert_eq!(sanitize_pct(f32::INFINITY), 0.0);
        assert_eq!(sanitize_pct(-5.0), 0.0);
        assert_eq!(sanitize_pct(145.0), 100.0);
        assert_eq!(sanitize_pct(23.46), 23.5);

        // 200% multi-core usage across 8 cores = 25.0% of total CPU.
        assert_eq!(normalize_process_cpu(200.0, 8), 25.0);
        assert_eq!(normalize_process_cpu(150.0, 0), 100.0);
        assert_eq!(normalize_process_cpu(f32::NAN, 8), 0.0);
    }

    #[test]
    fn reusable_sampler_collects_valid_live_snapshot() {
        let mut sampler = SystemOverviewSampler::default();
        let snap = sampler.sample().expect("live sample should succeed");
        assert!(snap.cpu_usage_pct.is_finite());
        assert!((0.0..=100.0).contains(&snap.cpu_usage_pct));
        assert!(snap.mem_used_bytes <= snap.mem_total_bytes);
        assert!(snap.top_processes.len() <= TOP_PROCESS_LIMIT);
        for pair in snap.top_processes.windows(2) {
            assert!(pair[0].memory_bytes >= pair[1].memory_bytes);
        }
    }
}
