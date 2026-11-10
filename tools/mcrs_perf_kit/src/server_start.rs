use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::bail;
use serde::Serialize;

use crate::median::median;
use crate::options::Options;
use crate::process::{Next, Process};
use crate::progress::Progress;
use crate::repo;
use crate::scratch_dir::ScratchDir;
use crate::section::Section;

const TIMEOUT: Duration = Duration::from_secs(120);
const PLAYING: &str = "entering Playing state";
const REGISTRIES: &str = "loaded registries";

#[derive(Debug, Serialize)]
pub struct ServerStartReport {
    pub playing_seconds_median: Option<f64>,
    pub registries_ms_median: Option<f64>,
    pub runs: Vec<StartRun>,
}

#[derive(Debug, Serialize)]
pub struct StartRun {
    pub run: usize,
    pub warm_up: bool,
    /// From the spawn to the `entering Playing state` line.
    pub playing_seconds: Option<f64>,
    /// The `elapsed` of the `loaded registries` line.
    pub registries_ms: Option<f64>,
    pub error: Option<String>,
}

pub fn run(options: &Options, errors: &mut Vec<String>) -> anyhow::Result<ServerStartReport> {
    let binary = repo::binary(&options.repo, "mcrs", "mcrs")?;
    let (warm_ups, measured) = if options.smoke { (0, 1) } else { (1, 3) };
    let total = warm_ups + measured;
    let mut progress = Progress::new(Section::ServerStart, total, Duration::from_secs(15));
    let mut runs = Vec::new();
    for run in 1..=total {
        let warm_up = run <= warm_ups;
        progress.step(&format!(
            "start {run}/{total}{}",
            if warm_up { " (warm-up)" } else { "" }
        ));
        let raw_log = options
            .out
            .join("raw")
            .join("server")
            .join(format!("start-{run}.log"));
        let mut report = StartRun {
            run,
            warm_up,
            playing_seconds: None,
            registries_ms: None,
            error: None,
        };
        match start(&binary, &options.repo, &raw_log) {
            Ok(timed) => {
                report.playing_seconds = Some(timed.playing_seconds);
                report.registries_ms = timed.registries_ms;
            }
            Err(error) => {
                errors.push(format!("server start {run}: {error:#}"));
                report.error = Some(format!("{error:#}"));
            }
        }
        runs.push(report);
    }
    let measured_runs = || runs.iter().filter(|run| !run.warm_up);
    let mut playing: Vec<f64> = measured_runs()
        .filter_map(|run| run.playing_seconds)
        .collect();
    let mut registries: Vec<f64> = measured_runs()
        .filter_map(|run| run.registries_ms)
        .collect();
    Ok(ServerStartReport {
        playing_seconds_median: median(&mut playing),
        registries_ms_median: median(&mut registries),
        runs,
    })
}

struct Timed {
    playing_seconds: f64,
    registries_ms: Option<f64>,
}

fn start(binary: &Path, repo: &Path, raw_log: &Path) -> anyhow::Result<Timed> {
    let dir = ScratchDir::new("server")?;
    let mut command = Command::new(binary);
    command
        .current_dir(dir.path())
        .env("BEVY_ASSET_ROOT", repo)
        .env("MCRS_WORLD_SEED", "12345")
        .env("MCRS_LAN_ANNOUNCE", "off")
        .env("MCRS_PORT", repo::free_port()?.to_string());
    let mut process = Process::spawn(command, raw_log)?;
    let result = wait_for_playing(&mut process);
    process.kill();
    result
}

fn wait_for_playing(process: &mut Process) -> anyhow::Result<Timed> {
    let deadline: Instant = process.started + TIMEOUT;
    let mut registries_ms = None;
    loop {
        let line = match process.next_line(deadline) {
            Next::Line(line) => line,
            Next::Timeout => bail!("no `{PLAYING}` line in {} s", TIMEOUT.as_secs()),
            Next::Exited => bail!("the server exited before `{PLAYING}`"),
        };
        if line.text.contains(REGISTRIES) {
            registries_ms = elapsed_ms(&line.text);
        }
        if line.text.contains(PLAYING) {
            return Ok(Timed {
                playing_seconds: line.at.duration_since(process.started).as_secs_f64(),
                registries_ms,
            });
        }
    }
}

/// The `elapsed=` field of a log line, a `Duration` printed with `Debug`, in milliseconds.
fn elapsed_ms(text: &str) -> Option<f64> {
    let (_, rest) = text.split_once("elapsed=")?;
    let value = rest.split_whitespace().next()?;
    let digits = value.trim_end_matches(|character: char| character.is_alphabetic());
    let number: f64 = digits.parse().ok()?;
    let scale = match value.get(digits.len()..)? {
        "s" => 1e3,
        "ms" => 1.0,
        "µs" | "us" => 1e-3,
        "ns" => 1e-6,
        _ => return None,
    };
    Some(number * scale)
}
