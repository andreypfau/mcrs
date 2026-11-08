use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, bail};

use crate::adapter::Kind;
use crate::phase::Phase;

/// Whether a watcher client joins the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    /// This harness's own protocol client, of mcrs's protocol version.
    Watcher,
    /// One `horizon-bots` client with the `stand` behaviour, for a server of 26.3's protocol.
    Bot,
    None,
}

pub struct Args {
    pub server: Kind,
    pub client: Client,
    pub binary: PathBuf,
    pub resources: Option<PathBuf>,
    pub bot: Option<PathBuf>,
    pub repo: PathBuf,
    pub dir: PathBuf,
    pub out: PathBuf,
    pub port: u16,
    pub threads: Option<usize>,
    pub phases: Vec<Phase>,
    pub pigs: usize,
    pub samples: usize,
    pub idle_warmup: Duration,
    pub warmup: Duration,
    pub measure: Duration,
    pub probe_interval: Duration,
    pub summon_rate: usize,
    pub sprint_ticks: u32,
    pub stimulus_pigs: usize,
    pub stimulus_period: Duration,
    pub stimulus_warmup: Duration,
    pub contended_cores: f64,
    pub java: String,
    pub heap: String,
}

const USAGE: &str = "usage: mob-bench run --server <mcrs|vanilla|horizon> --client <watcher|none> \
--binary <path> --dir <empty run folder> --out <report.json> [options]; see README.md";

fn seconds(value: &str) -> anyhow::Result<Duration> {
    Ok(Duration::from_secs_f64(value.parse()?))
}

pub fn parse(options: &[String]) -> anyhow::Result<Args> {
    let mut args = Args {
        server: Kind::Vanilla,
        client: Client::None,
        binary: PathBuf::new(),
        resources: None,
        bot: None,
        repo: std::env::current_dir()?,
        dir: PathBuf::new(),
        out: PathBuf::new(),
        port: 25610,
        threads: None,
        phases: vec![Phase::Idle, Phase::Roam, Phase::Crowd, Phase::Stimulus],
        pigs: 50_000,
        samples: 32,
        idle_warmup: Duration::from_secs(10),
        warmup: Duration::from_secs(30),
        measure: Duration::from_secs(60),
        probe_interval: Duration::from_secs(5),
        summon_rate: 5_000,
        sprint_ticks: 100,
        stimulus_pigs: 2_000,
        stimulus_period: Duration::from_secs(5),
        stimulus_warmup: Duration::from_secs(20),
        contended_cores: 1.0,
        java: "java".to_owned(),
        heap: "16g".to_owned(),
    };
    for pair in options.chunks(2) {
        let [name, value] = pair else {
            bail!("option {pair:?} has no value; {USAGE}");
        };
        match name.as_str() {
            "--server" => args.server = value.parse()?,
            "--client" => {
                args.client = match value.as_str() {
                    "watcher" => Client::Watcher,
                    "bot" => Client::Bot,
                    "none" => Client::None,
                    other => bail!("unknown client {other}"),
                }
            }
            "--binary" => args.binary = PathBuf::from(value),
            "--resources" => args.resources = Some(PathBuf::from(value)),
            "--bot" => args.bot = Some(PathBuf::from(value)),
            "--repo" => args.repo = PathBuf::from(value),
            "--dir" => args.dir = PathBuf::from(value),
            "--out" => args.out = PathBuf::from(value),
            "--port" => args.port = value.parse()?,
            "--threads" => args.threads = Some(value.parse()?),
            "--phases" => {
                args.phases = value
                    .split([',', ' '])
                    .filter(|phase| !phase.is_empty())
                    .map(str::parse)
                    .collect::<anyhow::Result<Vec<Phase>>>()?;
            }
            "--pigs" => args.pigs = value.parse()?,
            "--samples" => args.samples = value.parse()?,
            "--idle-warmup" => args.idle_warmup = seconds(value)?,
            "--warmup" => args.warmup = seconds(value)?,
            "--measure" => args.measure = seconds(value)?,
            "--probe-interval" => args.probe_interval = seconds(value)?,
            "--summon-rate" => args.summon_rate = value.parse()?,
            "--sprint-ticks" => args.sprint_ticks = value.parse()?,
            "--stimulus-pigs" => args.stimulus_pigs = value.parse()?,
            "--stimulus-period" => args.stimulus_period = seconds(value)?,
            "--stimulus-warmup" => args.stimulus_warmup = seconds(value)?,
            "--contended-cores" => args.contended_cores = value.parse()?,
            "--java" => args.java.clone_from(value),
            "--heap" => args.heap.clone_from(value),
            other => bail!("unknown option {other}; {USAGE}"),
        }
    }
    if args.binary.as_os_str().is_empty()
        || args.dir.as_os_str().is_empty()
        || args.out.as_os_str().is_empty()
    {
        bail!("{USAGE}");
    }
    std::fs::create_dir_all(&args.dir).with_context(|| format!("{}", args.dir.display()))?;
    Ok(args)
}
