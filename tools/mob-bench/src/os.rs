//! OS counters over a measured window: per-core utilisation, the server process, everything else.

use std::collections::HashMap;
use std::time::Instant;

use mcrs_minecraft_bench_support::round;
use serde::Serialize;

#[cfg(target_os = "macos")]
use crate::os_macos as platform;
#[cfg(windows)]
use crate::os_windows as platform;

#[derive(Clone, Copy, Debug, Default)]
pub struct CoreTicks {
    pub user: u64,
    pub system: u64,
    pub nice: u64,
    pub idle: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessUsage {
    pub cpu_nanos: u64,
    pub rss_bytes: u64,
}

pub struct Snapshot {
    at: Instant,
    cores: Vec<CoreTicks>,
    processes: HashMap<i32, u64>,
}

#[derive(Debug, Serialize)]
pub struct CpuReport {
    /// Busy share of each logical CPU over the window, in percent, in the kernel's CPU order.
    pub per_core_percent: Vec<f64>,
    /// The sum of the busy shares, as cores.
    pub busy_cores: f64,
    /// CPU time of the server process over the window, as cores.
    pub server_cores: f64,
    /// CPU time of this harness, its watcher and its client process over the window, as cores.
    pub harness_cores: f64,
    /// `busy_cores` less the server and the harness: every other process, the kernel included.
    pub other_cores: f64,
    /// The heaviest other processes that this user may read.
    pub top_other_processes: Vec<OtherProcess>,
}

#[derive(Debug, Serialize)]
pub struct OtherProcess {
    pub pid: i32,
    pub name: String,
    pub cores: f64,
}

const TOP_OTHERS: usize = 6;

#[must_use]
pub fn snapshot() -> Snapshot {
    let processes = pids()
        .into_iter()
        .filter_map(|pid| Some((pid, process(pid)?.cpu_nanos)))
        .collect();
    Snapshot {
        at: Instant::now(),
        cores: core_ticks(),
        processes,
    }
}

#[must_use]
pub fn cpu_between(
    start: &Snapshot,
    end: &Snapshot,
    server_pid: i32,
    client_pids: &[i32],
) -> CpuReport {
    let seconds = end.at.duration_since(start.at).as_secs_f64().max(1e-9);
    let per_core_percent: Vec<f64> = start
        .cores
        .iter()
        .zip(&end.cores)
        .map(|(before, after)| {
            let busy = (after.user + after.system + after.nice)
                .saturating_sub(before.user + before.system + before.nice);
            let idle = after.idle.saturating_sub(before.idle);
            round(busy as f64 * 100.0 / (busy + idle).max(1) as f64)
        })
        .collect();
    let busy_cores = per_core_percent.iter().sum::<f64>() / 100.0;
    let own_pid = std::process::id() as i32;
    let cores_of = |pid: i32| -> f64 {
        match (start.processes.get(&pid), end.processes.get(&pid)) {
            (Some(before), Some(after)) => after.saturating_sub(*before) as f64 / 1e9 / seconds,
            _ => 0.0,
        }
    };
    let server_cores = cores_of(server_pid);
    let harness_cores =
        cores_of(own_pid) + client_pids.iter().map(|pid| cores_of(*pid)).sum::<f64>();
    let mut others: Vec<OtherProcess> = end
        .processes
        .keys()
        .filter(|pid| **pid != server_pid && **pid != own_pid && !client_pids.contains(pid))
        .map(|pid| OtherProcess {
            pid: *pid,
            name: String::new(),
            cores: cores_of(*pid),
        })
        .filter(|other| other.cores > 0.0)
        .collect();
    others.sort_by(|left, right| right.cores.total_cmp(&left.cores));
    others.truncate(TOP_OTHERS);
    for other in &mut others {
        other.name = name(other.pid);
        other.cores = round(other.cores);
    }
    CpuReport {
        busy_cores: round(busy_cores),
        server_cores: round(server_cores),
        harness_cores: round(harness_cores),
        other_cores: round((busy_cores - server_cores - harness_cores).max(0.0)),
        per_core_percent,
        top_other_processes: others,
    }
}

#[cfg(any(target_os = "macos", windows))]
pub fn process(pid: i32) -> Option<ProcessUsage> {
    platform::process(pid)
}

#[cfg(any(target_os = "macos", windows))]
pub fn load_average() -> [f64; 3] {
    platform::load_average()
}

#[cfg(any(target_os = "macos", windows))]
fn core_ticks() -> Vec<CoreTicks> {
    platform::core_ticks()
}

#[cfg(any(target_os = "macos", windows))]
fn pids() -> Vec<i32> {
    platform::pids()
}

#[cfg(any(target_os = "macos", windows))]
fn name(pid: i32) -> String {
    platform::name(pid)
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn process(_pid: i32) -> Option<ProcessUsage> {
    None
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn load_average() -> [f64; 3] {
    [0.0; 3]
}

#[cfg(not(any(target_os = "macos", windows)))]
fn core_ticks() -> Vec<CoreTicks> {
    Vec::new()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn pids() -> Vec<i32> {
    Vec::new()
}

#[cfg(not(any(target_os = "macos", windows)))]
fn name(_pid: i32) -> String {
    String::new()
}
