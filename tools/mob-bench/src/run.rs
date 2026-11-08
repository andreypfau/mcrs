//! One run: one server, one client mode, the phases in order.

use std::collections::HashMap;
use std::net::TcpListener;
use std::net::{Ipv4Addr, SocketAddr};
use std::process::{Child, Command};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Context;
use glam::DVec3;

use crate::adapter::{LaunchOptions, TickMetric};
use crate::args::{Args, Client};
use crate::os::{self, round};
use crate::phase::Phase;
use crate::probe;
use crate::process::ServerProcess;
use crate::report::{
    self, Answered, External, GameTimeAnswer, Machine, Motion, PhaseReport, RunReport, Sighting,
    Sprint,
};
use crate::scene::{self, Area, PEN, PEN_WATCH, ROAM};
use crate::status;
use crate::watcher::{PigSnapshot, Watcher};

const READY_TIMEOUT: Duration = Duration::from_secs(600);
const CONSOLE_TIMEOUT: Duration = Duration::from_secs(20);
const JOIN_TIMEOUT: Duration = Duration::from_secs(60);
const POPULATE_TIMEOUT: Duration = Duration::from_secs(900);
const ANSWER_SLACK: Duration = Duration::from_secs(3);
const STOP_GRACE: Duration = Duration::from_secs(90);
/// The watcher hovers this high above the floor, out of the pigs' way.
const HOVER: f64 = 2.0;

struct Bench<'a> {
    args: &'a Args,
    runtime: tokio::runtime::Runtime,
    server: ServerProcess,
    watcher: Option<Watcher>,
    bot: Option<Child>,
    /// The name of the connected client's player, once it joined.
    player: Option<String>,
    /// Where the pigs stand now, `None` while there are none.
    pigs_in: Option<Area>,
    console: bool,
    metric: TickMetric,
    /// When the next stir of the stimulus phase is due, while that phase runs.
    next_stir: Option<Instant>,
}

/// The window of a measured phase, kept until the watcher's log is in.
struct Window {
    from: Instant,
    until: Instant,
}

pub fn run(args: &Args) -> anyhow::Result<RunReport> {
    args.server.prepare(&args.dir, args.port)?;
    let launch = args.server.launch(&LaunchOptions {
        binary: &args.binary,
        resources: args.resources.as_deref(),
        repo: &args.repo,
        java: &args.java,
        heap: &args.heap,
        port: args.port,
        threads: args.threads,
    })?;
    let started_unix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, launch.port));
    // A status answer from another server on the port would pass for this one's.
    TcpListener::bind((Ipv4Addr::UNSPECIFIED, launch.port))
        .with_context(|| format!("port {} is in use", launch.port))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let started = Instant::now();
    let server = ServerProcess::start(
        &launch.command,
        &launch.env,
        &args.dir,
        &args.dir.join("server.log"),
    )?;
    let mut bench = Bench {
        args,
        runtime,
        server,
        watcher: None,
        bot: None,
        player: None,
        pigs_in: None,
        console: false,
        metric: args.server.tick_metric(),
        next_stir: None,
    };
    let start_seconds = bench.wait_ready(address, started);
    let mut report = RunReport {
        server: args.server,
        client: match args.client {
            Client::Watcher => "watcher".to_owned(),
            Client::Bot => "horizon-bots stand".to_owned(),
            Client::None => "none".to_owned(),
        },
        threads: args.threads,
        threads_note: launch.threads_note,
        pigs: args.pigs,
        pen: PEN,
        roam: ROAM,
        tick_metric: bench.metric,
        start_seconds,
        console: false,
        settings: Vec::new(),
        watcher_error: None,
        machine: machine(),
        started_unix,
        command: launch.command,
        env: launch.env,
        phases: Vec::new(),
    };
    if start_seconds.is_none() {
        report.phases = skip_all(args, "the server did not answer a status request");
        bench.finish(&mut report, &[]);
        return Ok(report);
    }
    bench.console = bench.answers_console();
    report.console = bench.console;
    if bench.console {
        report.settings = bench.set_up();
    }
    let joined = match args.client {
        Client::Watcher => bench.join_watcher(address),
        Client::Bot => bench.join_bot(address),
        Client::None => Ok(()),
    };
    if let Err(error) = joined {
        report.watcher_error = Some(format!("{error:#}"));
        report.phases = skip_all(args, "the watcher did not join");
        bench.finish(&mut report, &[]);
        return Ok(report);
    }
    let mut windows = Vec::new();
    for phase in &args.phases {
        let (phase_report, window) = bench.phase(*phase);
        report.phases.push(phase_report);
        windows.push(window);
    }
    bench.finish(&mut report, &windows);
    Ok(report)
}

fn skip_all(args: &Args, reason: &str) -> Vec<PhaseReport> {
    args.phases
        .iter()
        .map(|phase| PhaseReport::skipped(*phase, reason.to_owned()))
        .collect()
}

impl Bench<'_> {
    fn wait_ready(&mut self, address: SocketAddr, started: Instant) -> Option<f64> {
        loop {
            self.server.pump();
            if self.server.exited() {
                return None;
            }
            if self.runtime.block_on(status::answers(address)) {
                return Some(round(started.elapsed().as_secs_f64()));
            }
            if started.elapsed() > READY_TIMEOUT {
                return None;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn answers_console(&mut self) -> bool {
        let sent = Instant::now();
        if self.server.send("time query gametime").is_err() {
            return false;
        }
        self.server
            .wait_for(sent, CONSOLE_TIMEOUT, probe::game_time)
            .is_some()
    }

    /// Sends one line and returns the first log line that follows it within a second.
    fn answered(&mut self, line: &str) -> Answered {
        let sent = Instant::now();
        let _ = self.server.send(line);
        let answer = self
            .server
            .wait_for(sent, Duration::from_secs(1), |text| Some(text.to_owned()));
        Answered {
            line: line.to_owned(),
            answer,
        }
    }

    fn set_up(&mut self) -> Vec<Answered> {
        let mut answers: Vec<Answered> = scene::settings()
            .iter()
            .map(|line| self.answered(line))
            .collect();
        let world: Vec<String> = [
            scene::forceload(PEN),
            scene::forceload(ROAM),
            scene::fence(PEN),
        ]
        .concat();
        answers.extend(world.iter().map(|line| self.answered(line)));
        self.idle_for(Duration::from_secs(5));
        answers
    }

    fn join_watcher(&mut self, address: SocketAddr) -> anyhow::Result<()> {
        let watcher = Watcher::spawn(&self.runtime, address);
        let deadline = Instant::now() + JOIN_TIMEOUT;
        while Instant::now() < deadline {
            if watcher.finished() {
                let log = watcher.stop(&self.runtime);
                anyhow::bail!(
                    log.error
                        .unwrap_or_else(|| "the watcher stopped".to_owned())
                );
            }
            if watcher
                .snapshot(&self.runtime)
                .is_some_and(|snapshot| snapshot.placed)
            {
                self.watcher = Some(watcher);
                self.player = Some(scene::WATCHER_NAME.to_owned());
                self.idle_for(Duration::from_secs(1));
                return Ok(());
            }
            self.idle_for(Duration::from_millis(500));
        }
        let _ = watcher.stop(&self.runtime);
        anyhow::bail!("{} was not placed in {JOIN_TIMEOUT:?}", scene::WATCHER_NAME)
    }

    fn join_bot(&mut self, address: SocketAddr) -> anyhow::Result<()> {
        let Some(program) = &self.args.bot else {
            anyhow::bail!("--client bot needs --bot, the horizon-bots binary");
        };
        let log = std::fs::File::create(self.args.dir.join("bot.log"))?;
        let bot = Command::new(program)
            .args([
                "--server",
                &address.to_string(),
                "--bots",
                "1",
                "--behaviour",
                "stand",
            ])
            .args(["--name", scene::WATCHER_NAME, "--seconds", "1000000"])
            .args(["--view-distance", "10"])
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .with_context(|| format!("starting {}", program.display()))?;
        self.bot = Some(bot);
        let name = format!("{}0", scene::WATCHER_NAME);
        let joined = format!("{name} joined the game");
        let sent = Instant::now();
        self.server
            .wait_for(sent, JOIN_TIMEOUT, |text| {
                text.contains(&joined).then_some(())
            })
            .with_context(|| format!("{name} did not join in {JOIN_TIMEOUT:?}"))?;
        self.player = Some(name);
        Ok(())
    }

    fn stir_when_due(&mut self) {
        let Some(due) = self.next_stir else {
            return;
        };
        if Instant::now() >= due {
            let _ = self.server.send(&scene::stimulus(self.args.stimulus_pigs));
            self.next_stir = Some(due + self.args.stimulus_period);
        }
    }

    fn idle_for(&mut self, duration: Duration) {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            self.stir_when_due();
            self.server.pump();
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn count_pigs(&mut self) -> Option<usize> {
        if !self.console {
            return None;
        }
        let sent = Instant::now();
        self.server.send(scene::COUNT_PIGS).ok()?;
        self.server
            .wait_for(sent, CONSOLE_TIMEOUT, probe::count_or_zero)
    }

    /// Puts the pigs into `area`, or removes them for `None`.
    fn populate(&mut self, area: Option<Area>) -> anyhow::Result<()> {
        if self.pigs_in == area {
            return Ok(());
        }
        if self.pigs_in.is_some() {
            self.server.send(scene::KILL_PIGS)?;
            self.wait_count(|count| count == 0)?;
            self.pigs_in = None;
        }
        let Some(area) = area else {
            return Ok(());
        };
        let lines = scene::summons(area, self.args.pigs, self.args.samples);
        let batch = (self.args.summon_rate / 20).max(1);
        for chunk in lines.chunks(batch) {
            for line in chunk {
                self.server.send(line)?;
            }
            self.server.pump();
            std::thread::sleep(Duration::from_millis(50));
        }
        let target = self.args.pigs;
        self.wait_count(|count| count >= target)?;
        self.pigs_in = Some(area);
        Ok(())
    }

    fn wait_count(&mut self, done: impl Fn(usize) -> bool) -> anyhow::Result<()> {
        let deadline = Instant::now() + POPULATE_TIMEOUT;
        let mut last = None;
        while Instant::now() < deadline {
            last = self.count_pigs();
            if last.is_some_and(&done) {
                return Ok(());
            }
            self.idle_for(Duration::from_secs(2));
        }
        anyhow::bail!("the pig count stayed at {last:?}")
    }

    fn phase(&mut self, phase: Phase) -> (PhaseReport, Option<Window>) {
        let setup_start = Instant::now();
        let needs_console = phase != Phase::Idle;
        if needs_console && !self.console {
            let reason = "the server answers no console command".to_owned();
            return (PhaseReport::skipped(phase, reason), None);
        }
        let area = match phase {
            Phase::Idle => None,
            Phase::Roam => Some(ROAM),
            Phase::Crowd | Phase::Stimulus => Some(PEN),
        };
        if self.console
            && let Err(error) = self.populate(area)
        {
            return (PhaseReport::skipped(phase, format!("{error:#}")), None);
        }
        let watch = match phase {
            Phase::Roam => ROAM.center() + DVec3::Y * HOVER,
            Phase::Idle | Phase::Crowd | Phase::Stimulus => PEN_WATCH,
        };
        if let Some(player) = self.player.clone() {
            let _ = self.server.send(&scene::teleport(&player, watch));
        }
        let pigs_before = self.count_pigs();
        let setup_seconds = round(setup_start.elapsed().as_secs_f64());
        let warmup = match phase {
            Phase::Idle => self.args.idle_warmup,
            Phase::Roam | Phase::Crowd => self.args.warmup,
            Phase::Stimulus => self.args.stimulus_warmup,
        };
        if phase == Phase::Stimulus {
            self.next_stir = Some(Instant::now());
        }
        self.idle_for(warmup);
        let (mut report, window) = self.measure(phase);
        self.next_stir = None;
        report.pigs_before = pigs_before;
        report.setup_seconds = setup_seconds;
        report.warmup_seconds = warmup.as_secs_f64();
        report.external.sprint = self.sprint();
        report.pigs_after = self.count_pigs();
        (report, Some(window))
    }

    fn probe_round(&mut self, first: bool) {
        if !self.console {
            return;
        }
        let _ = self.server.send("time query gametime");
        if !first && self.metric == TickMetric::TickQuery {
            let _ = self.server.send("tick query");
        }
        for index in 0..self.args.samples {
            let _ = self.server.send(&scene::sample_query(index));
        }
    }

    fn measure(&mut self, phase: Phase) -> (PhaseReport, Window) {
        let load_average_start = os::load_average();
        let start_os = os::snapshot();
        let watcher_start = self.watcher_snapshot();
        let from = Instant::now();
        let until = from + self.args.measure;
        let mut next_probe = from;
        let mut next_sample = from;
        let mut rss = Vec::new();
        let mut load_max = load_average_start[0];
        let mut first = true;
        while Instant::now() < until {
            let now = Instant::now();
            if now >= next_probe {
                self.probe_round(first);
                first = false;
                next_probe += self.args.probe_interval;
            }
            self.stir_when_due();
            if now >= next_sample {
                if let Some(usage) = os::process(self.server.pid()) {
                    rss.push(usage.rss_bytes as f64 / (1024.0 * 1024.0));
                }
                load_max = load_max.max(os::load_average()[0]);
                next_sample += Duration::from_secs(1);
            }
            self.server.pump();
            std::thread::sleep(Duration::from_millis(20));
        }
        self.probe_round(false);
        let end_os = os::snapshot();
        let watcher_end = self.watcher_snapshot();
        let load_average_end = os::load_average();
        let until = Instant::now();
        self.idle_for(ANSWER_SLACK);
        let harness: Vec<i32> = self.bot.iter().map(|bot| bot.id() as i32).collect();
        let cpu = os::cpu_between(&start_os, &end_os, self.server.pid(), &harness);
        let contended = cpu.other_cores > self.args.contended_cores;
        let report = PhaseReport {
            phase,
            skipped: None,
            pigs_before: None,
            pigs_after: None,
            setup_seconds: 0.0,
            warmup_seconds: 0.0,
            measure_seconds: round(until.duration_since(from).as_secs_f64()),
            server_mspt: self.server_mspt(from, until),
            external: External {
                console_game_time: report::console_rate(&self.game_times(from, until)),
                ..External::default()
            },
            motion: Motion {
                console: report::console_motion(&self.sightings(from, until)),
                watcher: watcher_start
                    .as_ref()
                    .zip(watcher_end.as_ref())
                    .and_then(|(start, end)| report::watcher_motion(start, end)),
                watcher_move_packets_per_second: watcher_start
                    .as_ref()
                    .zip(watcher_end.as_ref())
                    .map(|(start, end)| {
                        let seconds = end.at.duration_since(start.at).as_secs_f64().max(1e-9);
                        round((end.move_packets - start.move_packets) as f64 / seconds)
                    }),
            },
            cpu: Some(cpu),
            server_rss_mib_mean: (!rss.is_empty())
                .then(|| round(rss.iter().sum::<f64>() / rss.len() as f64)),
            server_rss_mib_max: rss.iter().copied().reduce(f64::max).map(round),
            load_average_start,
            load_average_end,
            load_average_1m_max: round(load_max.max(load_average_end[0])),
            contended,
        };
        (report, Window { from, until })
    }

    fn watcher_snapshot(&self) -> Option<PigSnapshot> {
        self.watcher.as_ref()?.snapshot(&self.runtime)
    }

    fn server_mspt(&self, from: Instant, until: Instant) -> Option<report::Mspt> {
        match self.metric {
            TickMetric::TickQuery => {
                let lines = self.server.lines_between(from, until + ANSWER_SLACK);
                let windows = probe::tick_windows(lines.iter().map(|line| line.text.as_str()));
                report::mspt_of_windows(&windows)
            }
            TickMetric::McrsTrace => {
                let lines = self.server.lines_between(from, until);
                let ticks: Vec<f64> = lines
                    .iter()
                    .filter_map(|line| probe::mcrs_tick_millis(&line.text))
                    .collect();
                report::mspt_of_ticks(ticks)
            }
        }
    }

    fn game_times(&self, from: Instant, until: Instant) -> Vec<GameTimeAnswer> {
        self.server
            .lines_between(from, until + ANSWER_SLACK)
            .iter()
            .filter_map(|line| {
                Some(GameTimeAnswer {
                    at: line.at,
                    game_time: probe::game_time(&line.text)?,
                })
            })
            .collect()
    }

    fn sightings(&self, from: Instant, until: Instant) -> HashMap<usize, Vec<Sighting>> {
        let mut samples: HashMap<usize, Vec<Sighting>> = HashMap::new();
        for line in self.server.lines_between(from, until + ANSWER_SLACK) {
            if let Some(sample) = probe::sample_position(&line.text) {
                samples.entry(sample.index).or_default().push(Sighting {
                    at: line.at,
                    position: sample.position,
                });
            }
        }
        samples
    }

    fn sprint(&mut self) -> Option<Sprint> {
        let ticks = self.args.sprint_ticks;
        if ticks == 0 || !self.console || !self.args.server.reports_sprint() {
            return None;
        }
        let sent = Instant::now();
        self.server.send(&format!("tick sprint {ticks}")).ok()?;
        // No console line goes out until the sprint reports its end, and a server that never
        // reports it leaves the sprint unmeasured.
        let timeout = Duration::from_secs(60).max(Duration::from_millis(1500) * ticks);
        let server_ms = self.server.wait_for(sent, timeout, probe::sprint_millis)?;
        let wall = sent.elapsed().as_secs_f64() * 1e3;
        Some(Sprint {
            ticks,
            wall_ms_per_tick: round(wall / f64::from(ticks)),
            server_ms_per_tick: Some(server_ms),
        })
    }

    fn finish(mut self, report: &mut RunReport, windows: &[Option<Window>]) {
        if let Some(mut bot) = self.bot.take() {
            let _ = bot.kill();
            let _ = bot.wait();
        }
        let log = self
            .watcher
            .take()
            .map(|watcher| watcher.stop(&self.runtime));
        if let Some(log) = log {
            report.watcher_error.clone_from(&log.error);
            for (phase, window) in report.phases.iter_mut().zip(windows) {
                let Some(window) = window else {
                    continue;
                };
                let inside = |at: &Instant| *at >= window.from && *at < window.until;
                let times: Vec<_> = log
                    .times
                    .iter()
                    .copied()
                    .filter(|sample| inside(&sample.at))
                    .collect();
                phase.external.watcher_set_time = report::set_time_intervals(&times);
            }
        }
        self.server.stop(self.args.server.stop(), STOP_GRACE);
    }
}

fn sysctl(name: &str) -> String {
    let output = Command::new("sysctl").arg("-n").arg(name).output();
    output.map_or_else(
        |_| String::new(),
        |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    )
}

fn machine() -> Machine {
    let os = Command::new("uname")
        .args(["-srm"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .context("uname");
    Machine {
        cpu: sysctl("machdep.cpu.brand_string"),
        logical_cpus: sysctl("hw.logicalcpu"),
        performance_cpus: sysctl("hw.perflevel0.logicalcpu"),
        efficiency_cpus: sysctl("hw.perflevel1.logicalcpu"),
        memory_bytes: sysctl("hw.memsize"),
        os: os.unwrap_or_default(),
    }
}
