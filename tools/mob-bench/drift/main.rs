//! Runs a small fenced pen of pigs at the density of the 50,000-pig scene with one still player
//! outside the west fence, and writes the pig positions the player's client sees at a list of
//! game times. Each checkpoint writes two snapshots `window` ticks apart, so that the pigs that
//! walk can be told from the pigs that stand.

#![allow(
    dead_code,
    reason = "the shared modules of mob-bench carry items this tool does not use"
)]

#[path = "../src/adapter.rs"]
mod adapter;
#[path = "../src/probe.rs"]
mod probe;
#[path = "../src/process.rs"]
mod process;
#[path = "../src/scene.rs"]
mod scene;
#[path = "../src/status.rs"]
mod status;
#[path = "../src/watcher.rs"]
mod watcher;

use std::fmt::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use glam::DVec3;

use adapter::{Kind, LaunchOptions, PlayerMode};
use process::ServerProcess;
use scene::Area;
use watcher::{PigSnapshot, Watcher};

const CONSOLE_TIMEOUT: Duration = Duration::from_secs(30);
const READY_TIMEOUT: Duration = Duration::from_secs(600);
/// Pigs a block, as in the 50,000-pig pen of 250 × 250 blocks.
const DENSITY: f64 = 0.8;
/// How far the player stands outside the west fence and above the floor, as in the user's scene
/// at (-134.5, -50, 0.5) next to the pen of x -125..125.
const PLAYER_OUTSIDE: f64 = 8.5;
const PLAYER_ABOVE: f64 = 10.0;

struct Options {
    server: Kind,
    binary: PathBuf,
    port: u16,
    dir: PathBuf,
    side: i32,
    checkpoints: Vec<i64>,
    window: i64,
    heap: String,
}

fn parse() -> anyhow::Result<Options> {
    let mut options = Options {
        server: Kind::Mcrs,
        binary: PathBuf::new(),
        port: 25800,
        dir: PathBuf::from("target/drift-run"),
        side: 80,
        checkpoints: vec![0, 1200, 3000, 6000],
        window: 200,
        heap: "8g".to_owned(),
    };
    let all: Vec<String> = std::env::args().skip(1).collect();
    for pair in all.chunks(2) {
        let [key, value] = pair else {
            bail!("options come in pairs");
        };
        match key.as_str() {
            "--server" => options.server = value.parse()?,
            "--binary" => options.binary = PathBuf::from(value),
            "--port" => options.port = value.parse()?,
            "--dir" => options.dir = PathBuf::from(value),
            "--side" => options.side = value.parse()?,
            "--window" => options.window = value.parse()?,
            "--heap" => options.heap.clone_from(value),
            "--checkpoints" => {
                options.checkpoints = value.split(',').map(str::parse).collect::<Result<_, _>>()?;
            }
            other => bail!("unknown option {other}"),
        }
    }
    Ok(options)
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let options = parse()?;
    std::fs::create_dir_all(&options.dir)?;
    let dir = options.dir.canonicalize()?;
    options
        .server
        .prepare(&dir, options.port, PlayerMode::Creative)?;
    let repo = std::env::current_dir()?;
    let mut launch = options.server.launch(&LaunchOptions {
        binary: &options.binary,
        resources: None,
        repo: &repo,
        java: "java",
        heap: &options.heap,
        port: options.port,
        threads: None,
        player_mode: PlayerMode::Creative,
    })?;
    for (key, value) in &mut launch.env {
        if key == "RUST_LOG" {
            "info".clone_into(value);
        }
    }
    TcpListener::bind((Ipv4Addr::UNSPECIFIED, options.port))
        .with_context(|| format!("port {} is in use", options.port))?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, options.port));
    let runtime = tokio::runtime::Runtime::new()?;
    let server = ServerProcess::start(&launch.command, &launch.env, &dir, &dir.join("server.log"))?;
    let mut drift = Drift {
        server,
        runtime,
        watcher: None,
    };
    let result = drift.run(&options, &dir, address);
    if let Some(watcher) = drift.watcher.take() {
        let log = watcher.stop(&drift.runtime);
        if let Some(error) = log.error {
            eprintln!("watcher: {error}");
        }
    }
    drift
        .server
        .stop(options.server.stop(), Duration::from_secs(60));
    result
}

struct Drift {
    server: ServerProcess,
    runtime: tokio::runtime::Runtime,
    watcher: Option<Watcher>,
}

impl Drift {
    fn run(
        &mut self,
        options: &Options,
        dir: &std::path::Path,
        address: SocketAddr,
    ) -> anyhow::Result<()> {
        let started = Instant::now();
        loop {
            self.server.pump();
            if self.server.exited() {
                bail!("the server exited");
            }
            if self.runtime.block_on(status::answers(address)) {
                break;
            }
            if started.elapsed() > READY_TIMEOUT {
                bail!("the server did not answer a status request");
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        self.game_time().context("the console does not answer")?;
        let half = options.side / 2;
        let pen = Area {
            min_x: -half,
            min_z: -half,
            size: options.side,
        };
        let lines = [scene::settings(), scene::forceload(pen), scene::fence(pen)].concat();
        for line in &lines {
            self.answered(line);
        }
        self.idle(Duration::from_secs(5));

        self.join_watcher(address)?;
        let player = DVec3::new(
            f64::from(pen.min_x) - 1.0 - PLAYER_OUTSIDE,
            f64::from(scene::FLOOR_Y) + PLAYER_ABOVE,
            0.5,
        );
        self.answered(&format!(
            "tp {} {} {} {} -90 0",
            scene::WATCHER_NAME,
            player.x,
            player.y,
            player.z
        ));
        self.idle(Duration::from_secs(2));

        let count = (DENSITY * f64::from(options.side * options.side)).round() as usize;
        let summons = scene::summons(pen, 1, count, 0, scene::Ai::On);
        for chunk in summons.chunks(100) {
            for line in chunk {
                self.server.send(line)?;
            }
            self.idle(Duration::from_millis(50));
        }
        let wait = Instant::now() + Duration::from_secs(300);
        loop {
            let sent = Instant::now();
            self.server.send(scene::COUNT_PIGS)?;
            let counted = self
                .server
                .wait_for(sent, CONSOLE_TIMEOUT, probe::count_or_zero);
            if counted.is_some_and(|counted| counted >= count) {
                break;
            }
            if Instant::now() > wait {
                bail!("only {counted:?} of {count} pigs");
            }
            self.idle(Duration::from_secs(1));
        }
        let start = self.game_time().context("no game time")?;
        let mut meta = format!(
            "server={:?}\nside={}\npigs={count}\nplayer={} {} {}\nstart={start}\nwindow={}\n",
            options.server, options.side, player.x, player.y, player.z, options.window
        );
        for &checkpoint in &options.checkpoints {
            let now = self.game_time().context("no game time")?;
            let ahead = start + checkpoint - now;
            if ahead > 0 {
                self.sprint(ahead)?;
            }
            self.idle(Duration::from_secs(2));
            let first_time = self.game_time().context("no game time")?;
            let first = self.snapshot()?;
            let until = first_time + options.window;
            while self.game_time().is_some_and(|now| now < until) {
                self.idle(Duration::from_millis(250));
            }
            let second_time = self.game_time().context("no game time")?;
            let second = self.snapshot()?;
            write_csv(&dir.join(format!("pigs-{checkpoint}.csv")), &first, &second)?;
            let _ = writeln!(
                meta,
                "checkpoint={checkpoint} first={} second={} seen={}",
                first_time - start,
                second_time - start,
                first.pigs.len()
            );
            eprintln!("checkpoint {checkpoint}: {} pigs seen", first.pigs.len());
        }
        std::fs::write(dir.join("meta.txt"), meta)?;
        Ok(())
    }

    fn join_watcher(&mut self, address: SocketAddr) -> anyhow::Result<()> {
        let watcher = Watcher::spawn(&self.runtime, address);
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if watcher.finished() {
                let log = watcher.stop(&self.runtime);
                bail!("the watcher stopped: {:?}", log.error);
            }
            if watcher
                .snapshot(&self.runtime)
                .is_some_and(|snapshot| snapshot.placed)
            {
                break;
            }
            if Instant::now() > deadline {
                bail!("the watcher was not placed");
            }
            self.idle(Duration::from_millis(500));
        }
        self.watcher = Some(watcher);
        Ok(())
    }

    fn snapshot(&self) -> anyhow::Result<PigSnapshot> {
        self.watcher
            .as_ref()
            .and_then(|watcher| watcher.snapshot(&self.runtime))
            .context("the watcher stopped")
    }

    fn sprint(&mut self, ticks: i64) -> anyhow::Result<()> {
        let sent = Instant::now();
        self.server.send(&format!("tick sprint {ticks}"))?;
        let timeout = Duration::from_secs(60) + Duration::from_millis(200) * ticks as u32;
        if self
            .server
            .wait_for(sent, timeout, probe::sprint_millis)
            .is_none()
        {
            bail!("no sprint report after {ticks} ticks");
        }
        Ok(())
    }

    fn game_time(&mut self) -> Option<i64> {
        let sent = Instant::now();
        self.server.send("time query gametime").ok()?;
        self.server
            .wait_for(sent, CONSOLE_TIMEOUT, probe::game_time)
    }

    fn answered(&mut self, line: &str) {
        let sent = Instant::now();
        let _ = self.server.send(line);
        let _ = self
            .server
            .wait_for(sent, Duration::from_secs(1), |text| Some(text.to_owned()));
    }

    fn idle(&mut self, duration: Duration) {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            self.server.pump();
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn write_csv(
    path: &std::path::Path,
    first: &PigSnapshot,
    second: &PigSnapshot,
) -> anyhow::Result<()> {
    let mut text = String::from("id,x,y,z,x2,y2,z2,path\n");
    for (id, pig) in &first.pigs {
        let Some(later) = second.pigs.get(id) else {
            continue;
        };
        let _ = writeln!(
            text,
            "{id},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3}",
            pig.position.x,
            pig.position.y,
            pig.position.z,
            later.position.x,
            later.position.y,
            later.position.z,
            later.path - pig.path
        );
    }
    std::fs::write(path, text)?;
    Ok(())
}
