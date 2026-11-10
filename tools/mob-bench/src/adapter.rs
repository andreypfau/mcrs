//! What differs between the servers: the files a run folder needs, the command line, the
//! thread knob, the tick metric and the way to stop.

use std::path::Path;
use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

use mcrs_minecraft_bench_support::{Kind, Stop, TickMetric};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Mcrs,
    Vanilla,
    Horizon,
}

impl FromStr for Target {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "mcrs" => Target::Mcrs,
            "vanilla" => Target::Vanilla,
            "horizon" => Target::Horizon,
            other => bail!("unknown server {other}"),
        })
    }
}

/// The game mode a joining player gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerMode {
    /// Does not reset the idle time of a pig and is not a target of its look goal, so the client
    /// of a run has no influence on the pigs.
    Spectator,
    Creative,
}

impl FromStr for PlayerMode {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "spectator" => PlayerMode::Spectator,
            "creative" => PlayerMode::Creative,
            other => bail!("unknown player mode {other}"),
        })
    }
}

impl PlayerMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            PlayerMode::Spectator => "spectator",
            PlayerMode::Creative => "creative",
        }
    }
}

pub struct Launch {
    pub command: Vec<String>,
    pub env: Vec<(String, String)>,
    /// How the thread count was applied, or why it was not.
    pub threads_note: String,
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
    pub player_mode: PlayerMode,
}

impl Target {
    /// The server of the shared harness that this one behaves like on its console and log.
    #[must_use]
    pub fn kind(self) -> Kind {
        match self {
            Target::Mcrs => Kind::Mcrs,
            Target::Vanilla | Target::Horizon => Kind::Vanilla,
        }
    }

    #[must_use]
    pub fn tick_metric(self) -> TickMetric {
        self.kind().tick_metric()
    }

    /// Whether `tick sprint` ends with a `Sprint completed` answer; Horizon's gives none.
    #[must_use]
    pub fn reports_sprint(self) -> bool {
        self != Target::Horizon
    }

    #[must_use]
    pub fn stop(self) -> Stop {
        self.kind().stop()
    }

    /// Writes the files the server reads at start into the empty folder `dir`.
    pub fn prepare(self, dir: &Path, port: u16, player_mode: PlayerMode) -> anyhow::Result<()> {
        if self == Target::Mcrs {
            return Ok(());
        }
        std::fs::write(dir.join("eula.txt"), "eula=true\n")?;
        std::fs::write(
            dir.join("server.properties"),
            server_properties(port, player_mode),
        )?;
        Ok(())
    }

    pub fn launch(self, options: &LaunchOptions<'_>) -> anyhow::Result<Launch> {
        let binary = options.binary.display().to_string();
        Ok(match self {
            Target::Vanilla => {
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
            Target::Horizon => {
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
            Target::Mcrs => {
                let mut env = [
                    ("MCRS_WORLD_SEED", "12345".to_owned()),
                    ("MCRS_WORLD_PRESET", "flat".to_owned()),
                    ("MCRS_COMPRESSION_THRESHOLD", "-1".to_owned()),
                    (
                        "MCRS_DEFAULT_GAMEMODE",
                        options.player_mode.as_str().to_owned(),
                    ),
                    ("MCRS_LAN_ANNOUNCE", "off".to_owned()),
                    ("BEVY_ASSET_ROOT", options.repo.display().to_string()),
                    ("RUST_LOG", "info,mcrs_tick=trace".to_owned()),
                    ("MCRS_PORT", options.port.to_string()),
                ]
                .map(|(key, value)| (key.to_owned(), value))
                .to_vec();
                let threads_note = match options.threads {
                    Some(threads) => {
                        env.push(("MCRS_COMPUTE_THREADS".to_owned(), threads.to_string()));
                        format!("MCRS_COMPUTE_THREADS={threads} (the Bevy compute pool)")
                    }
                    None => "Bevy default compute pool".to_owned(),
                };
                Launch {
                    command: vec![binary],
                    env,
                    threads_note,
                    port: options.port,
                }
            }
        })
    }
}

/// The settings both Java-format servers read. `pause-when-empty-seconds=-1` keeps the no-client
/// runs ticking; `max-tick-time=-1` keeps the watchdog from stopping a slow tick.
#[must_use]
pub fn server_properties(port: u16, player_mode: PlayerMode) -> String {
    let game_mode = player_mode.as_str();
    format!(
        "online-mode=false
server-port={port}
level-type=minecraft\\:flat
level-seed=12345
generate-structures=false
gamemode={game_mode}
force-gamemode=true
difficulty=easy
view-distance=10
simulation-distance=10
max-tick-time=-1
pause-when-empty-seconds=-1
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
