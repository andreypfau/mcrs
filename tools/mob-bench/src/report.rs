//! The JSON report of one run and the statistics its numbers come from.

use std::collections::HashMap;
use std::time::Instant;

use glam::DVec3;
use serde::Serialize;

use mcrs_minecraft_bench_support::{TickMetric, TickWindow, mean, nearest_rank, round};

use crate::adapter::Target;
use crate::os::CpuReport;
use crate::phase::Phase;
use crate::scene::Area;
use crate::watcher::{PigSnapshot, TimeSample};

/// A path shorter than this over the window counts as not moved: below the wire precision of
/// the deltas summed over a minute, and below the jitter of a pig pushed in place.
const MOVED_BLOCKS: f64 = 0.1;
/// A pig whose last position is this far from its first counts as displaced: the test of the
/// stroll experiment E-G13 of `docs/spec/mob-goals.md`.
const DISPLACED_BLOCKS: f64 = 0.5;

#[derive(Serialize)]
pub struct RunReport {
    pub server: Target,
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
    pub player_mode: Option<PlayerModeCheck>,
    pub stir_mode: crate::stir::Mode,
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

/// How the run set the game mode of its client's player, and what shows the mode.
#[derive(Serialize)]
pub struct PlayerModeCheck {
    pub command: Answered,
    /// The answer to `execute if entity @a[gamemode=spectator]`.
    pub spectators: Option<usize>,
    /// The game mode of the watcher's last login or game event packet.
    pub watcher_game_mode: Option<String>,
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
    pub stir: Option<StirReport>,
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
            stir: None,
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

/// The stir lines of the window and their answers.
#[derive(Serialize)]
pub struct StirReport {
    pub mode: crate::stir::Mode,
    /// What paces the lines: the game time from `time query gametime`.
    pub pacing: &'static str,
    /// Game ticks the stir served lines for.
    pub ticks_served: u64,
    pub lines_sent: u64,
    /// The `damage` lines of the panic hits, also answered with `Applied`.
    pub panic_lines_sent: u64,
    /// `Applied ... damage` answers: one a hit pig, of the stir and the panic hits.
    pub damage_answers: u64,
    pub damage_answers_per_second: f64,
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
    /// The mean distance between the first and the last position.
    pub mean_net_blocks: f64,
    /// The share of pigs whose last position is more than 0.5 blocks from the first.
    pub displaced_fraction: f64,
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
    let mut durations = Vec::new();
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
        nets.push(first.position.distance(last.position));
        durations.push(seconds);
    }
    summary(&paths, &nets, &durations, moved)
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
    let mut durations = Vec::new();
    let mut moved = 0;
    for (id, before) in &start.pigs {
        let Some(after) = end.pigs.get(id) else {
            continue;
        };
        let path = after.path - before.path;
        moved += usize::from(path > MOVED_BLOCKS);
        paths.push(path / seconds);
        nets.push(before.position.distance(after.position));
        durations.push(seconds);
    }
    summary(&paths, &nets, &durations, moved)
}

fn summary(paths: &[f64], nets: &[f64], durations: &[f64], moved: usize) -> Option<MotionSummary> {
    if paths.is_empty() {
        return None;
    }
    let per_second: Vec<f64> = nets
        .iter()
        .zip(durations)
        .map(|(net, seconds)| net / seconds)
        .collect();
    let displaced = nets.iter().filter(|net| **net > DISPLACED_BLOCKS).count();
    let four_places = |value: f64| (value * 1e4).round() / 1e4;
    Some(MotionSummary {
        pigs: paths.len(),
        moved_fraction: round(moved as f64 / paths.len() as f64),
        mean_path_blocks_per_second: four_places(mean(paths)),
        mean_net_blocks_per_second: four_places(mean(&per_second)),
        mean_net_blocks: four_places(mean(nets)),
        displaced_fraction: round(displaced as f64 / paths.len() as f64),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::watcher::PigTrack;

    #[test]
    fn mcrs_ticks_report_nearest_rank_percentiles_of_the_sorted_ticks() {
        let ticks: Vec<f64> = (1..=100).rev().map(f64::from).collect();
        let mspt = mspt_of_ticks(ticks).expect("100 ticks");
        assert_eq!(mspt.samples, 100);
        assert_eq!(mspt.mean, 50.5);
        assert_eq!(mspt.p50, 50.0);
        assert_eq!(mspt.p95, 95.0);
        assert_eq!(mspt.p99, 99.0);
        assert_eq!(mspt.worst, 100.0);
        assert!(mspt_of_ticks(Vec::new()).is_none());
    }

    #[test]
    fn tick_query_windows_average_per_field_and_keep_the_worst_p99() {
        let windows = [
            TickWindow {
                average: 10.0,
                p50: 9.0,
                p95: 20.0,
                p99: 30.0,
            },
            TickWindow {
                average: 20.0,
                p50: 11.0,
                p95: 40.0,
                p99: 50.0,
            },
        ];
        let mspt = mspt_of_windows(&windows).expect("two windows");
        assert_eq!(mspt.samples, 2);
        assert_eq!(mspt.mean, 15.0);
        assert_eq!(mspt.p50, 10.0);
        assert_eq!(mspt.p95, 30.0);
        assert_eq!(mspt.p99, 40.0);
        assert_eq!(mspt.worst, 50.0);
    }

    fn sighting(start: Instant, seconds: u64, x: f64) -> Sighting {
        Sighting {
            at: start + Duration::from_secs(seconds),
            position: DVec3::new(x, 0.0, 0.0),
        }
    }

    #[test]
    fn the_console_counts_the_pig_that_walks_and_not_the_one_that_stands() {
        let start = Instant::now();
        let samples = HashMap::from([
            (
                0,
                vec![
                    sighting(start, 0, 0.0),
                    sighting(start, 5, 5.0),
                    sighting(start, 10, 10.0),
                ],
            ),
            (1, vec![sighting(start, 0, 3.0), sighting(start, 10, 3.0)]),
        ]);
        let motion = console_motion(&samples).expect("two pigs");
        assert_eq!(motion.pigs, 2);
        assert_eq!(motion.moved_fraction, 0.5);
        assert_eq!(motion.displaced_fraction, 0.5);
        assert_eq!(motion.mean_path_blocks_per_second, 0.5);
        assert_eq!(motion.mean_net_blocks, 5.0);
    }

    #[test]
    fn the_watcher_counts_the_pig_that_walks_and_not_the_one_that_stands() {
        let at = Instant::now();
        let pig = |x: f64, path: f64| PigTrack {
            position: DVec3::new(x, 0.0, 0.0),
            path,
        };
        let snapshot = |at: Instant, walker: PigTrack| PigSnapshot {
            at,
            pigs: HashMap::from([(1, walker), (2, pig(3.0, 0.0))]),
            move_packets: 0,
            placed: true,
            game_mode: None,
        };
        let start = snapshot(at, pig(0.0, 0.0));
        let end = snapshot(at + Duration::from_secs(10), pig(10.0, 10.0));
        let motion = watcher_motion(&start, &end).expect("two pigs");
        assert_eq!(motion.pigs, 2);
        assert_eq!(motion.moved_fraction, 0.5);
        assert_eq!(motion.mean_path_blocks_per_second, 0.5);
        assert_eq!(motion.mean_net_blocks_per_second, 0.5);
        assert!(watcher_motion(&start, &start).is_none());
    }
}
