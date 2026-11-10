use std::net::TcpListener;
use std::path::{Path, PathBuf};

use std::time::Duration;

use serde::Serialize;

use crate::client_run::{Measured, SettleLimit, Timing, measure};
use crate::median::spread;
use crate::options::{Options, Window};
use crate::process::Process;
use crate::profile::Profile;
use crate::progress::Progress;
use crate::repo;
use crate::scratch_dir::ScratchDir;
use crate::section::Section;
use crate::shown_path;
use crate::stats_line::StatsLine;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scenario {
    /// No world: the client waits on a server that accepts the connection and never answers.
    Floor,
    Base,
    Flight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Window,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Clock {
    Cold,
    Hot,
}

#[derive(Debug, Serialize)]
pub struct ClientReport {
    pub adapter: Option<String>,
    pub scenarios: Vec<ScenarioReport>,
}

#[derive(Debug, Serialize)]
pub struct ScenarioReport {
    pub scenario: Scenario,
    pub mode: Mode,
    pub clock: Clock,
    /// The median of the run medians.
    pub median: StatsLine,
    pub frame_median_ms_spread: Option<f64>,
    pub engine_median_ms_spread: Option<f64>,
    pub runs: Vec<RunReport>,
}

#[derive(Debug, Serialize)]
pub struct RunReport {
    pub run: usize,
    pub lines: usize,
    pub settled: Option<bool>,
    pub settle_seconds: Option<f64>,
    pub medians: Option<StatsLine>,
    pub raw_log: PathBuf,
    pub error: Option<String>,
    /// Set when the OS gave the window another size than the one requested.
    pub window_mismatch: Option<String>,
}

fn window_mismatch(window: Window, medians: &StatsLine) -> Option<String> {
    let (Some(width), Some(height)) = (medians.physical_width, medians.physical_height) else {
        return None;
    };
    // Whole pixels: the stats line prints integers and the median of equal values is exact.
    if width.round() == f64::from(window.width) && height.round() == f64::from(window.height) {
        return None;
    }
    Some(format!(
        "requested {}, got {width:.0}x{height:.0}",
        window.resolution()
    ))
}

const SEED: &str = "12345";
const VIEW: &str = "16";
const FLY: &str = "0.05";

pub fn run(options: &Options, errors: &mut Vec<String>) -> anyhow::Result<ClientReport> {
    let binary = repo::binary(
        &options.repo,
        "mcrs_minecraft_client",
        "mcrs_minecraft_client",
    )?;
    // The client quits when its connection fails, so the floor needs a server that holds it.
    // chisle: the silent server lives until the kit exits, one thread for the whole section.
    let silent = TcpListener::bind("127.0.0.1:0")?;
    let floor_server = silent.local_addr()?.to_string();
    std::thread::spawn(move || {
        let held: Vec<_> = silent.incoming().map_while(Result::ok).collect();
        drop(held);
    });
    let modes: &[Mode] = match options.profile {
        Profile::Quick => &[Mode::Window],
        Profile::Full => &[Mode::Window, Mode::Fullscreen],
    };
    let mut launches = Vec::new();
    for scenario in [Scenario::Floor, Scenario::Base, Scenario::Flight] {
        for &mode in modes {
            for clock in [Clock::Cold, Clock::Hot] {
                launches.push(Launch {
                    options,
                    binary: &binary,
                    scenario,
                    mode,
                    clock,
                    floor_server: &floor_server,
                });
            }
        }
    }
    let runs = if options.smoke { 1 } else { 2 };
    let mut progress = Progress::new(
        Section::Client,
        launches.len() * runs,
        Duration::from_secs(80),
    );
    let mut report = ClientReport {
        adapter: None,
        scenarios: Vec::new(),
    };
    for launch in &launches {
        let label = launch.label();
        let mut run_reports = Vec::new();
        let mut medians = Vec::new();
        for run in 1..=runs {
            progress.step(&format!("{label} run {run}/{runs}"));
            let run_report = launch.run(run, &mut report.adapter);
            if let Some(error) = &run_report.error {
                errors.push(format!("client {label} run {run}: {error}"));
            }
            if let Some(mismatch) = &run_report.window_mismatch {
                println!("[client] warning: {label} run {run}: window {mismatch}");
            }
            medians.extend(run_report.medians.clone());
            run_reports.push(run_report);
        }
        report.scenarios.push(ScenarioReport {
            scenario: launch.scenario,
            mode: launch.mode,
            clock: launch.clock,
            median: StatsLine::median_of(&medians),
            frame_median_ms_spread: spread(medians.iter().filter_map(|line| line.frame_median_ms)),
            engine_median_ms_spread: spread(
                medians.iter().filter_map(|line| line.engine_median_ms),
            ),
            runs: run_reports,
        });
    }
    Ok(report)
}

struct Launch<'a> {
    options: &'a Options,
    binary: &'a Path,
    scenario: Scenario,
    mode: Mode,
    clock: Clock,
    floor_server: &'a str,
}

impl Launch<'_> {
    fn label(&self) -> String {
        format!(
            "{} {} {}",
            self.scenario.name(),
            self.mode.name(),
            self.clock.name()
        )
    }

    fn run(&self, run: usize, adapter: &mut Option<String>) -> RunReport {
        let raw_log = self.options.out.join("raw").join("client").join(format!(
            "{}-{}-{}-{run}.log",
            self.scenario.name(),
            self.mode.name(),
            self.clock.name()
        ));
        let mut report = RunReport {
            run,
            lines: 0,
            settled: None,
            settle_seconds: None,
            medians: None,
            raw_log: shown_path::under_out(&raw_log, &self.options.out),
            error: None,
            window_mismatch: None,
        };
        match self.measure(&raw_log) {
            Ok(measured) => {
                if adapter.is_none() {
                    *adapter = measured.adapter;
                }
                report.lines = measured.lines.len();
                report.settled = measured.settled;
                report.settle_seconds = measured.settle_seconds;
                let medians = StatsLine::median_of(&measured.lines);
                if self.mode == Mode::Window {
                    report.window_mismatch = window_mismatch(self.options.window, &medians);
                }
                report.medians = Some(medians);
            }
            Err(error) => report.error = Some(format!("{error:#}")),
        }
        report
    }

    fn measure(&self, raw_log: &Path) -> anyhow::Result<Measured> {
        let dir = ScratchDir::new("client")?;
        let mut command = crate::process::clean_command(self.binary);
        command
            .current_dir(dir.path())
            .env("BEVY_ASSET_ROOT", &self.options.repo)
            .env("MCRS_STATS", "1")
            .env("MCRS_VSYNC", "0");
        match self.scenario {
            Scenario::Floor => {
                command.env("MCRS_SERVER", self.floor_server);
            }
            Scenario::Base => {
                command.env("MCRS_WORLD_SEED", SEED).env("MCRS_VIEW", VIEW);
            }
            Scenario::Flight => {
                command
                    .env("MCRS_WORLD_SEED", SEED)
                    .env("MCRS_VIEW", VIEW)
                    .env("MCRS_FLY", FLY);
            }
        }
        match self.mode {
            Mode::Window => command.env("MCRS_RESOLUTION", self.options.window.resolution()),
            Mode::Fullscreen => command.env("MCRS_FULLSCREEN", "1"),
        };
        if self.clock == Clock::Hot {
            command.env("MCRS_HOT", "1");
        }
        measure(Process::spawn(command, raw_log)?, self.timing())
    }

    fn timing(&self) -> Timing {
        let seconds = Duration::from_secs;
        let smoke = self.options.smoke;
        let settle = (self.scenario == Scenario::Base).then_some(SettleLimit {
            lines: 5,
            cap: if smoke { seconds(30) } else { seconds(120) },
        });
        let measure = match (self.scenario, smoke) {
            (_, true) => seconds(5),
            (Scenario::Flight, false) => seconds(60),
            (Scenario::Floor | Scenario::Base, false) => seconds(30),
        };
        Timing {
            discard: if smoke { seconds(2) } else { seconds(10) },
            settle,
            measure,
        }
    }
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Floor => "floor",
            Self::Base => "base",
            Self::Flight => "flight",
        }
    }
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Fullscreen => "fullscreen",
        }
    }
}

impl Clock {
    fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Hot => "hot",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(width: f64, height: f64) -> StatsLine {
        StatsLine {
            physical_width: Some(width),
            physical_height: Some(height),
            ..StatsLine::default()
        }
    }

    #[test]
    fn a_shrunk_window_is_reported() {
        let window = Window {
            width: 1920,
            height: 1080,
        };
        assert_eq!(window_mismatch(window, &sized(1920.0, 1080.0)), None);
        assert_eq!(window_mismatch(window, &StatsLine::default()), None);
        assert_eq!(
            window_mismatch(window, &sized(1920.0, 1050.0)).as_deref(),
            Some("requested 1920x1080, got 1920x1050")
        );
    }
}
