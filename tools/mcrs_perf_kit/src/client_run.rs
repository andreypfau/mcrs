use std::time::{Duration, Instant};

use anyhow::bail;

use crate::process::{Next, Process};
use crate::settle::Settle;
use crate::stats_line::StatsLine;

/// The first start may download the client jar before the window opens.
const STARTUP: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// Dropped after the first stats line.
    pub discard: Duration,
    pub settle: Option<SettleLimit>,
    pub measure: Duration,
}

#[derive(Clone, Copy, Debug)]
pub struct SettleLimit {
    pub lines: usize,
    /// Counted from the first stats line.
    pub cap: Duration,
}

pub struct Measured {
    pub lines: Vec<StatsLine>,
    pub settled: Option<bool>,
    pub settle_seconds: Option<f64>,
    pub adapter: Option<String>,
}

/// Reads the stats lines of a running client over `timing`, then kills it.
pub fn measure(mut process: Process, timing: Timing) -> anyhow::Result<Measured> {
    let result = read(&mut process, timing);
    process.kill();
    result
}

fn read(process: &mut Process, timing: Timing) -> anyhow::Result<Measured> {
    let mut measured = Measured {
        lines: Vec::new(),
        settled: None,
        settle_seconds: None,
        adapter: None,
    };
    let mut settle = timing.settle.map(|limit| Settle::new(limit.lines));
    let settle_cap = timing.settle.map_or(Duration::ZERO, |limit| limit.cap);
    let mut first: Option<Instant> = None;
    let mut measure_from: Option<Instant> = None;
    loop {
        let deadline = match (first, measure_from) {
            (_, Some(from)) => from + timing.measure,
            (Some(first), None) => first + timing.discard + settle_cap + Duration::from_secs(10),
            (None, None) => process.started + STARTUP,
        };
        let line = match process.next_line(deadline) {
            Next::Line(line) => line,
            Next::Timeout if measure_from.is_some() => return Ok(measured),
            Next::Timeout if first.is_none() => bail!("no stats line in {} s", STARTUP.as_secs()),
            Next::Timeout => bail!("the client stopped logging stats lines"),
            Next::Exited => bail!(
                "the client exited after {:.0} s",
                process.started.elapsed().as_secs_f64()
            ),
        };
        if measured.adapter.is_none()
            && let Some(at) = line.text.find("AdapterInfo")
        {
            measured.adapter = line.text.get(at..).map(str::to_owned);
        }
        let Some(stats) = StatsLine::parse(&line.text) else {
            continue;
        };
        let first = *first.get_or_insert(line.at);
        if line.at < first + timing.discard {
            continue;
        }
        if let Some(from) = measure_from {
            if line.at >= from + timing.measure {
                return Ok(measured);
            }
            measured.lines.push(stats);
            continue;
        }
        let Some(watch) = &mut settle else {
            measure_from = Some(line.at);
            measured.lines.push(stats);
            continue;
        };
        let since_first = line.at.duration_since(first);
        let settled = watch.observe(&stats);
        if settled || since_first >= settle_cap {
            measured.settled = Some(settled);
            measured.settle_seconds = Some(since_first.as_secs_f64());
            measure_from = Some(line.at);
        }
    }
}
