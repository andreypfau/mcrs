//! What differs between the servers: the files a run folder needs, the command line, the
//! thread knob, the tick metric and the way to stop.

use std::path::Path;
use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

use crate::process::Stop;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mcrs,
    Vanilla,
    Horizon,
}

impl FromStr for Kind {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "mcrs" => Kind::Mcrs,
            "vanilla" => Kind::Vanilla,
            "horizon" => Kind::Horizon,
            other => bail!("unknown server {other}"),
        })
    }
}

/// Where a server reports its own tick times.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TickMetric {
    /// `tick query` on the console: the mean and percentiles of the last 100 ticks.
    TickQuery,
    /// One `mcrs_tick` trace line a tick.
    McrsTrace,
}

pub struct Launch {
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
    /// How the thread count was applied, or why it was not.
    pub threads_note: String,
    /// The port the server listens on, which mcrs cannot be told.
    pub port: u16,
}

pub struct LaunchOptions<'a> {
    pub binary: &'a Path,
    pub resources: Option<&'a Path>,
    pub repo: &'a Path,
    pub java: &'a str,
    pub heap: &'a str,
    pub port: u16,
    pub threads: Option<usize>,
}

/// The port mcrs binds; it has no setting for it.
pub const MCRS_PORT: u16 = 25565;

impl Kind {
    #[must_use]
    pub fn tick_metric(self) -> TickMetric {
        match self {
            Kind::Mcrs => TickMetric::McrsTrace,
            Kind::Vanilla | Kind::Horizon => TickMetric::TickQuery,
        }
    }

    #[must_use]
    pub fn stop(self) -> Stop {
        match self {
            Kind::Mcrs => Stop::Interrupt,
            Kind::Vanilla | Kind::Horizon => Stop::Console,
        }
    }

    /// Writes the files the server reads at start into the empty folder `dir`.
    pub fn prepare(self, dir: &Path, port: u16) -> anyhow::Result<()> {
        if self == Kind::Mcrs {
            return Ok(());
        }
        std::fs::write(dir.join("eula.txt"), "eula=true\n")?;
        std::fs::write(dir.join("server.properties"), server_properties(port))?;
        Ok(())
    }

    pub fn launch(self, options: &LaunchOptions<'_>) -> anyhow::Result<Launch> {
        let binary = options.binary.display().to_string();
        Ok(match self {
            Kind::Vanilla => {
                let mut command = vec![
                    options.java.to_owned(),
                    format!("-Xms{}", options.heap),
                    format!("-Xmx{}", options.heap),
                ];
                let threads_note = match options.threads {
                    Some(threads) => {
                        command.push(format!("-XX:ActiveProcessorCount={threads}"));
                        format!(
                            "-XX:ActiveProcessorCount={threads} (the JVM sizes GC, JIT and common pools by it; the server tick stays on one thread)"
                        )
                    }
                    None => "JVM default (all CPUs)".to_owned(),
                };
                command.extend(["-jar".to_owned(), binary, "nogui".to_owned()]);
                Launch {
                    command,
                    env: Vec::new(),
                    threads_note,
                    port: options.port,
                }
            }
            Kind::Horizon => {
                let Some(resources) = options.resources else {
                    bail!("horizon needs --resources, the folder horizon-prepare wrote");
                };
                if options.threads.is_some() {
                    bail!("horizon documents no worker-thread setting");
                }
                Launch {
                    command: vec![
                        binary,
                        "--resources".to_owned(),
                        resources.display().to_string(),
                        "--dedicated".to_owned(),
                        "--nogui".to_owned(),
                        "--port".to_owned(),
                        options.port.to_string(),
                    ],
                    env: Vec::new(),
                    threads_note: "no documented thread setting; defaults".to_owned(),
                    port: options.port,
                }
            }
            Kind::Mcrs => {
                if options.threads.is_some() {
                    bail!("mcrs has no compute thread count setting yet");
                }
                let env = [
                    ("MCRS_WORLD_SEED", "12345".to_owned()),
                    ("MCRS_WORLD_PRESET", "flat".to_owned()),
                    ("MCRS_COMPRESSION_THRESHOLD", "-1".to_owned()),
                    ("MCRS_DEFAULT_GAMEMODE", "creative".to_owned()),
                    ("MCRS_LAN_ANNOUNCE", "off".to_owned()),
                    ("BEVY_ASSET_ROOT", options.repo.display().to_string()),
                    ("RUST_LOG", "info,mcrs_tick=trace".to_owned()),
                ]
                .map(|(key, value)| (key.to_owned(), value))
                .to_vec();
                Launch {
                    command: vec![binary],
                    env,
                    threads_note: "no compute thread setting; Bevy defaults".to_owned(),
                    port: MCRS_PORT,
                }
            }
        })
    }
}

/// The settings both Java-format servers read. `pause-when-empty-seconds=0` keeps the no-client
/// runs ticking; `max-tick-time=-1` keeps the watchdog from stopping a slow tick.
#[must_use]
pub fn server_properties(port: u16) -> String {
    format!(
        "online-mode=false
server-port={port}
level-type=minecraft\\:flat
level-seed=12345
generate-structures=false
gamemode=creative
force-gamemode=true
difficulty=easy
view-distance=10
simulation-distance=10
max-tick-time=-1
pause-when-empty-seconds=0
spawn-protection=0
max-players=10
enforce-secure-profile=false
network-compression-threshold=-1
sync-chunk-writes=true
entity-broadcast-range-percentage=100
white-list=false
"
    )
}
