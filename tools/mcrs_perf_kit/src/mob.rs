use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::Serialize;

use crate::java;
use crate::mob_summary::{MobPhase, RunIn};
use crate::options::Options;
use crate::profile::Profile;
use crate::progress::Progress;
use crate::repo;
use crate::scratch_dir::ScratchDir;
use crate::section::Section;
use crate::shown_path;

const JAVA_MAJOR: u32 = 25;
const QUICK_PHASES: &str = "idle,crowd-still,crowd-stir";
const FULL_PHASES: &str =
    "idle,roam-still,roam-stir,crowd-still,crowd-stir,stimulus,cost-still,cost-stir";

#[derive(Debug, Serialize)]
pub struct MobRun {
    pub name: String,
    pub report: PathBuf,
    pub phases: Vec<MobPhase>,
    pub error: Option<String>,
}

struct Target {
    server: &'static str,
    binary: PathBuf,
}

#[derive(Clone, Copy)]
enum ClientMode {
    Off,
    Watcher,
}

impl ClientMode {
    /// The `--client` value mob-bench takes.
    fn as_str(self) -> &'static str {
        match self {
            Self::Off => "none",
            Self::Watcher => "watcher",
        }
    }
}

struct PhaseSeconds {
    idle_warmup: &'static str,
    warmup: &'static str,
    measure: &'static str,
}

pub fn run(options: &Options, errors: &mut Vec<String>) -> anyhow::Result<Vec<MobRun>> {
    if !options.accept_eula {
        bail!(
            "the mob section runs the vanilla server: read https://aka.ms/MinecraftEULA and pass --accept-eula"
        );
    }
    let major = java::major_version(&options.java)?;
    if major < JAVA_MAJOR {
        bail!(
            "{} is Java {major}; the vanilla server needs Java {JAVA_MAJOR} or later (--java <path>)",
            options.java
        );
    }
    let bench = repo::binary(&options.repo, "mob-bench", "mcrs_mob_bench")?;
    let targets = [
        Target {
            server: "mcrs",
            binary: repo::binary(&options.repo, "mcrs", "mcrs")?,
        },
        Target {
            server: "vanilla",
            binary: crate::vanilla_jar::server_jar(&options.repo)?,
        },
    ];
    let clients: &[ClientMode] = match options.profile {
        Profile::Quick => &[ClientMode::Off],
        Profile::Full => &[ClientMode::Watcher, ClientMode::Off],
    };
    let mut progress = Progress::new(
        Section::Mob,
        targets.len() * clients.len(),
        Duration::from_secs(600),
    );
    let mut runs = Vec::new();
    for target in &targets {
        for &client in clients {
            let client_name = client.as_str();
            let name = format!("{}-{client_name}", target.server);
            progress.step(&format!("{} with client {client_name}", target.server));
            let report = options.out.join("mob").join(format!("{name}.json"));
            let mut run = MobRun {
                name,
                report: shown_path::under_out(&report, &options.out),
                phases: Vec::new(),
                error: None,
            };
            match run_one(options, &bench, target, client, &run.name) {
                Ok(phases) => run.phases = phases,
                Err(error) => {
                    errors.push(format!("mob {}: {error:#}", run.name));
                    run.error = Some(format!("{error:#}"));
                }
            }
            runs.push(run);
        }
    }
    Ok(runs)
}

fn run_one(
    options: &Options,
    bench: &Path,
    target: &Target,
    client: ClientMode,
    name: &str,
) -> anyhow::Result<Vec<MobPhase>> {
    let dir = ScratchDir::new(&format!("mob-{name}"))?;
    let report = options.out.join("mob").join(format!("{name}.json"));
    let raw = options.out.join("raw").join("mob");
    std::fs::create_dir_all(&raw)?;
    if let Some(parent) = report.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let stdout = File::create(raw.join(format!("{name}.stdout")))?;
    let stderr = stdout.try_clone()?;
    let status = crate::process::clean_command(bench)
        .args(arguments(options, target, client, dir.path(), &report)?)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .status()
        .context("running mob-bench")?;
    let _ = std::fs::copy(
        dir.path().join("server.log"),
        raw.join(format!("{name}.server.log")),
    );
    if !status.success() {
        bail!("mob-bench exited with {status}; see raw/mob/{name}.stdout");
    }
    let text = std::fs::read(&report).with_context(|| format!("reading {}", report.display()))?;
    let run: RunIn = serde_json::from_slice(&text)?;
    Ok(run.into_phases().collect())
}

/// The arguments `tools/mob-bench/run.sh` builds, with its defaults.
fn arguments(
    options: &Options,
    target: &Target,
    client: ClientMode,
    dir: &Path,
    report: &Path,
) -> anyhow::Result<Vec<String>> {
    let phases = match (options.smoke, options.profile) {
        (true, _) => "idle",
        (false, Profile::Quick) => QUICK_PHASES,
        (false, Profile::Full) => FULL_PHASES,
    };
    let seconds = if options.smoke {
        PhaseSeconds {
            idle_warmup: "2",
            warmup: "2",
            measure: "5",
        }
    } else {
        PhaseSeconds {
            idle_warmup: "10",
            warmup: "30",
            measure: "60",
        }
    };
    let path = |path: &Path| path.to_string_lossy().into_owned();
    let pairs = [
        ("--server", target.server.to_owned()),
        ("--client", client.as_str().to_owned()),
        ("--repo", path(&options.repo)),
        ("--dir", path(dir)),
        ("--out", path(report)),
        ("--port", repo::free_port()?.to_string()),
        ("--phases", phases.to_owned()),
        ("--pigs", "50000".to_owned()),
        ("--samples", "32".to_owned()),
        ("--idle-warmup", seconds.idle_warmup.to_owned()),
        ("--warmup", seconds.warmup.to_owned()),
        ("--measure", seconds.measure.to_owned()),
        ("--probe-interval", "5".to_owned()),
        ("--summon-rate", "5000".to_owned()),
        ("--sprint-ticks", "100".to_owned()),
        ("--stimulus-pigs", "2000".to_owned()),
        ("--stimulus-period", "5".to_owned()),
        ("--stimulus-warmup", "20".to_owned()),
        ("--stir-mode", "uuid".to_owned()),
        ("--player-mode", "spectator".to_owned()),
        ("--contended-cores", "1.0".to_owned()),
        ("--java", options.java.clone()),
        ("--heap", options.heap.clone()),
        ("--binary", path(&target.binary)),
    ];
    let mut arguments = vec!["run".to_owned()];
    arguments.extend(
        pairs
            .into_iter()
            .flat_map(|(flag, value)| [flag.to_owned(), value]),
    );
    Ok(arguments)
}
