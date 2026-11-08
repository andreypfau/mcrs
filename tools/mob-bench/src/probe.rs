//! Parsers for the console answers and log lines the harness reads.

use glam::DVec3;

use crate::scene::SAMPLE_PREFIX;

/// The marker of the line the mcrs server loop logs after each tick, at trace level.
const MCRS_TICK: &str = "mcrs_tick: micros=";
const AVERAGE: &str = "Average time per tick: ";
const PERCENTILES: &str = "Percentiles: P50: ";
/// 26.4 answers `The game time is 1234 tick(s)`; earlier versions `The time is 1234`.
const GAME_TIME: [&str; 2] = ["The game time is ", "The time is "];
/// 26.4 answers `Test passed. Count: 2`; earlier versions `Test passed, count: 2`.
const COUNT: [&str; 2] = ["Test passed. Count: ", "Test passed, count: "];
const ENTITY_DATA: &str = " has the following entity data: ";
const SPRINT: &str = "Sprint completed with ";

/// One answer to `tick query`: the mean and percentiles of the last 100 ticks, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickWindow {
    pub average: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

/// `Average time per tick: 0.1ms (Target: 50.0ms)` followed by
/// `Percentiles: P50: 0.1ms P95: 0.1ms P99: 1.6ms. Sample: 100`.
pub fn tick_windows<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<TickWindow> {
    let mut windows = Vec::new();
    let mut average = None;
    for line in lines {
        if let Some((_, rest)) = line.split_once(AVERAGE) {
            average = millis(rest);
        } else if let Some((_, rest)) = line.split_once(PERCENTILES) {
            let mut parts = rest.split(" P9").map(millis);
            let (Some(average), Some(Some(p50)), Some(Some(p95)), Some(Some(p99))) =
                (average.take(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            windows.push(TickWindow {
                average,
                p50,
                p95,
                p99,
            });
        }
    }
    windows
}

/// The number before the first `ms`, after an optional `5: ` or `9: ` label.
fn millis(text: &str) -> Option<f64> {
    let (number, _) = text.split_once("ms")?;
    number.rsplit(' ').next()?.parse().ok()
}

/// The duration of one mcrs tick from its `mcrs_tick` trace line, in milliseconds.
#[must_use]
pub fn mcrs_tick_millis(line: &str) -> Option<f64> {
    let (_, micros) = line.split_once(MCRS_TICK)?;
    let micros: f64 = micros.trim().parse().ok()?;
    Some(micros / 1e3)
}

/// The answer to `time query gametime`.
#[must_use]
pub fn game_time(line: &str) -> Option<i64> {
    let (_, rest) = GAME_TIME
        .iter()
        .find_map(|marker| line.split_once(marker))?;
    rest.split(' ').next()?.trim().parse().ok()
}

/// The answer to `execute if entity` when something matches.
#[must_use]
pub fn count(line: &str) -> Option<usize> {
    let (_, rest) = COUNT.iter().find_map(|marker| line.split_once(marker))?;
    rest.trim().parse().ok()
}

/// `Test failed`, the answer to `execute if entity` when nothing matches.
#[must_use]
pub fn count_or_zero(line: &str) -> Option<usize> {
    if line.trim_end().ends_with("Test failed") {
        return Some(0);
    }
    count(line)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplePosition {
    pub index: usize,
    pub position: DVec3,
}

/// `mbs7 has the following entity data: [1.5d, -60.0d, 3.25d]`.
#[must_use]
pub fn sample_position(line: &str) -> Option<SamplePosition> {
    let (head, data) = line.split_once(ENTITY_DATA)?;
    let (_, index) = head.rsplit_once(SAMPLE_PREFIX)?;
    let index: usize = index.parse().ok()?;
    let inner = data.trim().strip_prefix('[')?.strip_suffix(']')?;
    let mut axes = inner
        .split(',')
        .map(|axis| axis.trim().trim_end_matches('d').parse::<f64>().ok());
    let (Some(Some(x)), Some(Some(y)), Some(Some(z))) = (axes.next(), axes.next(), axes.next())
    else {
        return None;
    };
    Some(SamplePosition {
        index,
        position: DVec3::new(x, y, z),
    })
}

/// The milliseconds a tick of `Sprint completed with 20.5 ticks per second, or 48.7 ms per tick`.
#[must_use]
pub fn sprint_millis(line: &str) -> Option<f64> {
    let (_, rest) = line.split_once(SPRINT)?;
    let (_, rest) = rest.split_once(", or ")?;
    let (number, _) = rest.split_once(" ms")?;
    number.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_query_answer_gives_one_window() {
        let lines = [
            "[12:00:00] [Server thread/INFO]: Average time per tick: 118.4ms (Target: 50.0ms)",
            "[12:00:00] [Server thread/INFO]: Percentiles: P50: 117.0ms P95: 130.2ms P99: 151.9ms. Sample: 100",
        ];
        assert_eq!(
            tick_windows(lines),
            [TickWindow {
                average: 118.4,
                p50: 117.0,
                p95: 130.2,
                p99: 151.9,
            }]
        );
    }

    #[test]
    fn the_game_time_and_the_count_read_in_both_wordings() {
        let at = "[00:23:32] [Server thread/INFO]: System chat: ";
        assert_eq!(
            game_time(&format!("{at}The game time is 4 tick(s)")),
            Some(4)
        );
        assert_eq!(game_time(&format!("{at}The time is 4")), Some(4));
        assert_eq!(count(&format!("{at}Test passed. Count: 2")), Some(2));
        assert_eq!(count_or_zero(&format!("{at}Test failed")), Some(0));
    }

    #[test]
    fn a_sample_answer_names_its_pig() {
        let line = "[12:00:00] [Server thread/INFO]: mbs17 has the following entity data: [12.5d, -60.0d, -3.25d]";
        assert_eq!(
            sample_position(line),
            Some(SamplePosition {
                index: 17,
                position: DVec3::new(12.5, -60.0, -3.25),
            })
        );
    }
}
