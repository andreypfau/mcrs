//! The JSON report of one run and the statistics its numbers come from.

use std::collections::HashMap;
use std::time::Instant;

use glam::DVec3;
use serde::Serialize;

use crate::adapter::{Kind, TickMetric};
use crate::os::{CpuReport, round};
use crate::phase::Phase;
use crate::probe::TickWindow;
use crate::scene::Area;
use crate::watcher::{PigSnapshot, TimeSample};

/// A path shorter than this over the window counts as not moved: below the wire precision of
/// the deltas summed over a minute, and below the jitter of a pig pushed in place.
const MOVED_BLOCKS: f64 = 0.1;

#[derive(Serialize)]
pub struct RunReport {
    pub server: Kind,
    pub client: String,
    pub threads: Option<usize>,
    pub threads_note: String,
    pub pigs: usize,
    pub pen: Area,
    pub roam: Area,
    pub tick_metric: TickMetric,
    pub start_seconds: Option<f64>,
    pub console: bool,
    pub settings: Vec<Answered>,
    pub watcher_error: Option<String>,
    pub machine: Machine,
    pub started_unix: u64,
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
    pub phases: Vec<PhaseReport>,
}

#[derive(Serialize)]
pub struct Answered {
    pub line: String,
    pub answer: Option<String>,
}

#[derive(Serialize, Default)]
pub struct Machine {
    pub cpu: String,
    pub logical_cpus: String,
    pub performance_cpus: String,
    pub efficiency_cpus: String,
    pub memory_bytes: String,
    pub os: String,
}

#[derive(Serialize)]
pub struct PhaseReport {
    pub phase: Phase,
    pub skipped: Option<String>,
    pub pigs_before: Option<usize>,
    pub pigs_after: Option<usize>,
    pub setup_seconds: f64,
    pub warmup_seconds: f64,
    pub measure_seconds: f64,
    pub server_mspt: Option<Mspt>,
    pub external: External,
    pub motion: Motion,
    pub cpu: Option<CpuReport>,
    pub server_rss_mib_mean: Option<f64>,
    pub server_rss_mib_max: Option<f64>,
    pub load_average_start: [f64; 3],
    pub load_average_end: [f64; 3],
    pub load_average_1m_max: f64,
    pub contended: bool,
    /// Probe rounds not sent because the game time answer of the round before was still out.
    pub probe_rounds_skipped: usize,
}

impl PhaseReport {
    #[must_use]
    pub fn skipped(phase: Phase, reason: String) -> Self {
        Self {
            phase,
            skipped: Some(reason),
            pigs_before: None,
            pigs_after: None,
            setup_seconds: 0.0,
            warmup_seconds: 0.0,
            measure_seconds: 0.0,
            server_mspt: None,
            external: External::default(),
            motion: Motion::default(),
            cpu: None,
            server_rss_mib_mean: None,
            server_rss_mib_max: None,
            load_average_start: [0.0; 3],
            load_average_end: [0.0; 3],
            load_average_1m_max: 0.0,
            contended: false,
            probe_rounds_skipped: 0,
        }
    }
}

/// The server's own tick times over the window, in milliseconds.
#[derive(Serialize)]
pub struct Mspt {
    /// `tick query`: answers, each over the last 100 ticks; `mcrs_tick`: ticks.
    pub samples: usize,
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    /// The highest P99 of one `tick query` answer; for `mcrs_tick` the longest tick.
    pub worst: f64,
}

#[derive(Serialize, Default)]
pub struct External {
    /// Game time from `time query gametime` against the arrival of its answers.
    pub console_game_time: Option<Rate>,
    /// Game time from the watcher's `set_time` packets against their arrival: one sample per
    /// pair of packets (vanilla sends one every 20 ticks).
    pub watcher_set_time: Option<Intervals>,
    /// `tick sprint` timed from the command to its answer.
    pub sprint: Option<Sprint>,
}

#[derive(Serialize)]
pub struct Rate {
    pub ticks: i64,
    pub seconds: f64,
    pub tps: f64,
    pub mean_ms_per_tick: f64,
}

#[derive(Serialize)]
pub struct Intervals {
    pub samples: usize,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub tps: f64,
}

#[derive(Serialize)]
pub struct Sprint {
    pub ticks: u32,
    pub wall_ms_per_tick: f64,
    pub server_ms_per_tick: Option<f64>,
}

#[derive(Serialize, Default)]
pub struct Motion {
    /// The named sample pigs, from `data get entity` every probe interval.
    pub console: Option<MotionSummary>,
    /// Every pig the watcher tracked at both ends of the window.
    pub watcher: Option<MotionSummary>,
    pub watcher_move_packets_per_second: Option<f64>,
}

#[derive(Serialize)]
pub struct MotionSummary {
    pub pigs: usize,
    pub moved_fraction: f64,
    /// The mean path length a second: the sum of the steps between position samples.
    pub mean_path_blocks_per_second: f64,
    /// The mean distance between the first and the last position, a second.
    pub mean_net_blocks_per_second: f64,
}

#[must_use]
pub fn nearest_rank(sorted: &[f64], quantile: f64) -> f64 {
    let index = (quantile * sorted.len() as f64).ceil() as usize;
    sorted.get(index.saturating_sub(1)).copied().unwrap_or(0.0)
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[must_use]
pub fn mspt_of_windows(windows: &[TickWindow]) -> Option<Mspt> {
    if windows.is_empty() {
        return None;
    }
    let pick = |field: fn(&TickWindow) -> f64| -> f64 {
        round(windows.iter().map(field).sum::<f64>() / windows.len() as f64)
    };
    Some(Mspt {
        samples: windows.len(),
        mean: pick(|window| window.average),
        p50: pick(|window| window.p50),
        p95: pick(|window| window.p95),
        p99: pick(|window| window.p99),
        worst: windows.iter().map(|window| window.p99).fold(0.0, f64::max),
    })
}

#[must_use]
pub fn mspt_of_ticks(mut ticks: Vec<f64>) -> Option<Mspt> {
    if ticks.is_empty() {
        return None;
    }
    ticks.sort_by(f64::total_cmp);
    Some(Mspt {
        samples: ticks.len(),
        mean: round(mean(&ticks)),
        p50: round(nearest_rank(&ticks, 0.50)),
        p95: round(nearest_rank(&ticks, 0.95)),
        p99: round(nearest_rank(&ticks, 0.99)),
        worst: round(ticks.last().copied().unwrap_or(0.0)),
    })
}

#[derive(Clone, Copy)]
pub struct GameTimeAnswer {
    pub at: Instant,
    pub game_time: i64,
}

#[must_use]
pub fn console_rate(answers: &[GameTimeAnswer]) -> Option<Rate> {
    let (first, last) = (answers.first()?, answers.last()?);
    let seconds = last.at.duration_since(first.at).as_secs_f64();
    let ticks = last.game_time - first.game_time;
    if seconds <= 0.0 || ticks <= 0 {
        return None;
    }
    Some(Rate {
        ticks,
        seconds: round(seconds),
        tps: round(ticks as f64 / seconds),
        mean_ms_per_tick: round(seconds * 1e3 / ticks as f64),
    })
}

#[must_use]
pub fn set_time_intervals(samples: &[TimeSample]) -> Option<Intervals> {
    let per_tick: Vec<f64> = samples
        .windows(2)
        .filter_map(|pair| {
            let [before, after] = pair else {
                return None;
            };
            let ticks = after.game_time - before.game_time;
            (ticks > 0)
                .then(|| after.at.duration_since(before.at).as_secs_f64() * 1e3 / ticks as f64)
        })
        .collect();
    let first = samples.first()?;
    let last = samples.last()?;
    let ticks = last.game_time - first.game_time;
    let seconds = last.at.duration_since(first.at).as_secs_f64();
    intervals(per_tick, (seconds > 0.0).then(|| ticks as f64 / seconds))
}

fn intervals(mut values: Vec<f64>, tps: Option<f64>) -> Option<Intervals> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(Intervals {
        samples: values.len(),
        mean_ms: round(mean(&values)),
        p50_ms: round(nearest_rank(&values, 0.50)),
        p95_ms: round(nearest_rank(&values, 0.95)),
        p99_ms: round(nearest_rank(&values, 0.99)),
        tps: round(tps.unwrap_or(0.0)),
    })
}

#[derive(Clone, Copy)]
pub struct Sighting {
    pub at: Instant,
    pub position: DVec3,
}

/// Motion of the named pigs from their position samples.
#[must_use]
pub fn console_motion(samples: &HashMap<usize, Vec<Sighting>>) -> Option<MotionSummary> {
    let mut paths = Vec::new();
    let mut nets = Vec::new();
    let mut moved = 0;
    for sightings in samples.values() {
        let (Some(first), Some(last)) = (sightings.first(), sightings.last()) else {
            continue;
        };
        let seconds = last.at.duration_since(first.at).as_secs_f64();
        if seconds <= 0.0 {
            continue;
        }
        let path: f64 = sightings
            .windows(2)
            .filter_map(|pair| match pair {
                [before, after] => Some(before.position.distance(after.position)),
                _ => None,
            })
            .sum();
        moved += usize::from(path > MOVED_BLOCKS);
        paths.push(path / seconds);
        nets.push(first.position.distance(last.position) / seconds);
    }
    summary(&paths, &nets, moved)
}

/// Motion of the pigs the watcher tracked at both ends of the window.
#[must_use]
pub fn watcher_motion(start: &PigSnapshot, end: &PigSnapshot) -> Option<MotionSummary> {
    let seconds = end.at.duration_since(start.at).as_secs_f64();
    if seconds <= 0.0 {
        return None;
    }
    let mut paths = Vec::new();
    let mut nets = Vec::new();
    let mut moved = 0;
    for (id, before) in &start.pigs {
        let Some(after) = end.pigs.get(id) else {
            continue;
        };
        let path = after.path - before.path;
        moved += usize::from(path > MOVED_BLOCKS);
        paths.push(path / seconds);
        nets.push(before.position.distance(after.position) / seconds);
    }
    summary(&paths, &nets, moved)
}

fn summary(paths: &[f64], nets: &[f64], moved: usize) -> Option<MotionSummary> {
    if paths.is_empty() {
        return None;
    }
    Some(MotionSummary {
        pigs: paths.len(),
        moved_fraction: round(moved as f64 / paths.len() as f64),
        mean_path_blocks_per_second: (mean(paths) * 1e4).round() / 1e4,
        mean_net_blocks_per_second: (mean(nets) * 1e4).round() / 1e4,
    })
}
