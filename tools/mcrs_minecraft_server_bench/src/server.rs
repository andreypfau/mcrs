use std::fs;
use std::time::Instant;

use mcrs_minecraft_bench_support::{
    Kind, Line, TickWindow, mcrs_tick_millis, mcrs_windows, tick_windows,
};

const LINUX_CLOCK_TICKS_PER_SECOND: f64 = 100.0;

/// The resident memory in bytes and the CPU time in seconds of `root` and its descendants, read
/// from `/proc`; `None` where there is no `/proc`.
pub fn process_tree(root: u32) -> Option<Usage> {
    let mut stats: Vec<ProcStat> = Vec::new();
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if let Some(stat) = proc_stat(pid) {
            stats.push(stat);
        }
    }
    let mut tree = vec![root];
    let mut index = 0;
    while let Some(&parent) = tree.get(index) {
        tree.extend(
            stats
                .iter()
                .filter(|stat| stat.ppid == parent)
                .map(|stat| stat.pid),
        );
        index += 1;
    }
    let mut usage = Usage::default();
    for stat in stats.iter().filter(|stat| tree.contains(&stat.pid)) {
        usage.cpu_seconds += stat.cpu_ticks as f64 / LINUX_CLOCK_TICKS_PER_SECOND;
        usage.rss_bytes += stat.rss_bytes;
    }
    Some(usage)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    pub rss_bytes: u64,
    pub cpu_seconds: f64,
}

struct ProcStat {
    pid: u32,
    ppid: u32,
    cpu_ticks: u64,
    rss_bytes: u64,
}

fn proc_stat(pid: u32) -> Option<ProcStat> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name may hold spaces and parentheses; the fields after it do not.
    let fields: Vec<&str> = stat.get(stat.rfind(')')? + 2..)?.split(' ').collect();
    let field = |index: usize| fields.get(index)?.parse::<u64>().ok();
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let rss_kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.trim().trim_end_matches(" kB").parse::<u64>().ok())
        .unwrap_or(0);
    Some(ProcStat {
        pid,
        ppid: u32::try_from(field(1)?).ok()?,
        cpu_ticks: field(11)? + field(12)?,
        rss_bytes: rss_kib * 1024,
    })
}

/// The tick windows that close inside `from..until`: for vanilla the answers to `/tick query`,
/// for mcrs the logged ticks cut into windows of the same size.
pub fn windows(kind: Kind, log: &[Line], from: Instant, until: Instant) -> Vec<TickWindow> {
    let inside = log
        .iter()
        .filter(|line| line.at >= from && line.at < until)
        .map(|line| line.text.as_str());
    match kind {
        Kind::Vanilla => tick_windows(inside),
        Kind::Mcrs => {
            let ticks: Vec<f64> = inside.filter_map(mcrs_tick_millis).collect();
            mcrs_windows(&ticks)
        }
    }
}
