//! Tick times from the two server logs, and the rank and rounding maths of the reports.

/// The marker of the line the mcrs server loop logs after each tick, at trace level.
const MCRS_TICK: &str = "mcrs_tick: micros=";
const AVERAGE: &str = "Average time per tick: ";
const PERCENTILES: &str = "Percentiles: P50: ";
/// The window of one vanilla `tick query`, so the mcrs ticks are cut into windows of the same size.
pub const WINDOW_TICKS: usize = 100;

/// The mean and percentiles of the tick times of one window, in milliseconds.
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

/// The full windows of [`WINDOW_TICKS`] ticks in `ticks`, in milliseconds; a short tail is dropped.
#[must_use]
pub fn mcrs_windows(ticks: &[f64]) -> Vec<TickWindow> {
    ticks
        .as_chunks::<WINDOW_TICKS>()
        .0
        .iter()
        .map(|chunk| {
            let mut sorted = *chunk;
            sorted.sort_by(f64::total_cmp);
            TickWindow {
                average: mean(&sorted),
                p50: nearest_rank(&sorted, 0.50),
                p95: nearest_rank(&sorted, 0.95),
                p99: nearest_rank(&sorted, 0.99),
            }
        })
        .collect()
}

/// The value at `quantile` of the ascending `sorted`, by the nearest-rank rule.
#[must_use]
pub fn nearest_rank(sorted: &[f64], quantile: f64) -> f64 {
    let index = (quantile * sorted.len() as f64).ceil() as usize;
    sorted.get(index.saturating_sub(1)).copied().unwrap_or(0.0)
}

#[must_use]
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[must_use]
pub fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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
    fn the_trace_line_of_a_tick_reads_in_milliseconds() {
        let line = "2026-01-01T00:00:00Z TRACE mcrs_tick: micros=2500";
        assert_eq!(mcrs_tick_millis(line), Some(2.5));
        assert_eq!(mcrs_tick_millis("an unrelated line"), None);
    }

    #[test]
    fn nearest_rank_picks_the_ceiling_rank() {
        let sorted = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(nearest_rank(&sorted, 0.50), 5.0);
        assert_eq!(nearest_rank(&sorted, 0.51), 6.0);
        assert_eq!(nearest_rank(&sorted, 0.95), 10.0);
        assert_eq!(nearest_rank(&sorted, 1.0), 10.0);
        assert_eq!(nearest_rank(&sorted, 0.0), 1.0);
        assert_eq!(nearest_rank(&[], 0.5), 0.0);
    }

    #[test]
    fn mcrs_ticks_are_cut_into_whole_windows() {
        let ticks: Vec<f64> = (1..=250).map(|tick| f64::from(tick % 100 + 1)).collect();
        let windows = mcrs_windows(&ticks);
        assert_eq!(windows.len(), 2);
        let window = windows[0];
        assert_eq!(window.p50, 50.0);
        assert_eq!(window.p95, 95.0);
        assert_eq!(window.p99, 99.0);
        assert_eq!(window.average, 50.5);
    }
}
