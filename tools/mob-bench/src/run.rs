//! One run: one server, one client mode, the phases in order.

use std::collections::{HashMap, VecDeque};
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
    self, Answered, External, GameTimeAnswer, Machine, Motion, PhaseReport, PlayerModeCheck,
    RunReport, Sighting, Sprint, StirReport,
};
use crate::scene::{self, Ai, Area, PEN, PEN_WATCH, ROAM};
use crate::status;
use crate::stir::{self, Stir};
use crate::watcher::{PigSnapshot, Watcher};

const READY_TIMEOUT: Duration = Duration::from_secs(600);
const CONSOLE_TIMEOUT: Duration = Duration::from_secs(20);
const JOIN_TIMEOUT: Duration = Duration::from_secs(60);
const POPULATE_TIMEOUT: Duration = Duration::from_secs(900);
/// A pig count that has not changed for this long will not reach its target.
const STALL_TIMEOUT: Duration = Duration::from_secs(60);
const ANSWER_SLACK: Duration = Duration::from_secs(3);
const STOP_GRACE: Duration = Duration::from_secs(90);
/// The watcher hovers this high above the floor, out of the pigs' way.
const HOVER: f64 = 2.0;
/// The least ticks of a warm-up: one `tick query` answer covers the last 100 ticks.
const WARMUP_TICKS: i64 = 120;
/// The share of the panic period over which one round's lines go out.
const PANIC_SPREAD: f64 = 0.8;
/// Blocks beyond the open area that the stir strips cover: the pigs stroll out of it.
const ROAM_STIR_MARGIN: i32 = 32;
/// A stir clock query with no answer for this long is sent again.
const STIR_QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// The pigs standing in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Population {
    area: Area,
    ai: Ai,
}

struct Bench<'a> {
    args: &'a Args,
    runtime: tokio::runtime::Runtime,
    server: ServerProcess,
    watcher: Option<Watcher>,
    bot: Option<Child>,
    /// The name of the connected client's player, once it joined.
    player: Option<String>,
    /// Where the pigs stand now, `None` while there are none.
    pigs_in: Option<Population>,
    console: bool,
    metric: TickMetric,
    /// The population the pigs now standing belong to; their UUIDs carry it.
    generation: u32,
    panic: PanicHits,
    stir: StirClock,
    /// When the last probe round went out; the next waits for its game time answer.
    probe_sent: Option<Instant>,
    probe_rounds_skipped: usize,
}

/// The panic hits of the stimulus phase.
struct PanicHits {
    /// When the next round is due, while the phase runs.
    next: Option<Instant>,
    round: usize,
    /// The lines of the current round not sent yet: a round goes out over most of its period,
    /// so no single tick takes all of it.
    lines: VecDeque<String>,
    started: Instant,
    total: usize,
    sent: u64,
}

#[derive(Default)]
struct StirClock {
    /// The stir of a stirred phase while it runs.
    stir: Option<Stir>,
    /// When the last `time query gametime` of the stir went out, while its answer is due.
    query: Option<Instant>,
    /// The first game tick the stir has not served lines for yet.
    next_tick: Option<i64>,
    ticks_served: u64,
    lines_sent: u64,
}

struct StirCounts {
    ticks_served: u64,
    lines_sent: u64,
    panic_lines_sent: u64,
    damage_answers: u64,
}

/// The window of a measured phase, kept until the watcher's log is in.
struct Window {
    from: Instant,
    until: Instant,
}

pub fn run(args: &Args) -> anyhow::Result<RunReport> {
    args.server
        .prepare(&args.dir, args.port, args.player_mode)?;
    let launch = args.server.launch(&LaunchOptions {
        binary: &args.binary,
        resources: args.resources.as_deref(),
        repo: &args.repo,
        java: &args.java,
        heap: &args.heap,
        port: args.port,
        threads: args.threads,
        player_mode: args.player_mode,
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
    let mut bench = Bench::new(args, runtime, server);
    let start_seconds = bench.wait_ready(address, started);
    let mut report = RunReport {
        server: args.server,
        client: client_label(args.client).to_owned(),
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
        player_mode: None,
        stir_mode: args.stir_mode,
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
    report.player_mode = bench.set_player_mode();
    let mut windows = Vec::new();
    for phase in &args.phases {
        let (phase_report, window) = bench.phase(*phase);
        report.phases.push(phase_report);
        windows.push(window);
    }
    bench.finish(&mut report, &windows);
    Ok(report)
}

fn client_label(client: Client) -> &'static str {
    match client {
        Client::Watcher => "watcher",
        Client::Bot => "horizon-bots stand",
        Client::None => "none",
    }
}

fn skip_all(args: &Args, reason: &str) -> Vec<PhaseReport> {
    args.phases
        .iter()
        .map(|phase| PhaseReport::skipped(*phase, reason.to_owned()))
        .collect()
}

impl<'a> Bench<'a> {
    fn new(args: &'a Args, runtime: tokio::runtime::Runtime, server: ServerProcess) -> Self {
        Self {
            args,
            runtime,
            server,
            watcher: None,
            bot: None,
            player: None,
            pigs_in: None,
            console: false,
            metric: args.server.tick_metric(),
            generation: 0,
            panic: PanicHits {
                next: None,
                round: 0,
                lines: VecDeque::new(),
                started: Instant::now(),
                total: 0,
                sent: 0,
            },
            stir: StirClock::default(),
            probe_sent: None,
            probe_rounds_skipped: 0,
        }
    }
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

    /// Sends one line and returns the first log line that follows it within a second, other than
    /// the mcrs tick trace.
    fn answered(&mut self, line: &str) -> Answered {
        let sent = Instant::now();
        let _ = self.server.send(line);
        let answer = self
            .server
            .wait_for(sent, Duration::from_secs(1), |text| {
                probe::mcrs_tick_millis(text)
                    .is_none()
                    .then(|| text.to_owned())
            });
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

    /// Sets the game mode of the client's player, also where the server default already did, and
    /// checks it.
    fn set_player_mode(&mut self) -> Option<PlayerModeCheck> {
        let player = self.player.clone()?;
        let command = self.answered(&scene::game_mode(&player, self.args.player_mode));
        let spectators = if self.console {
            let sent = Instant::now();
            let _ = self.server.send(scene::COUNT_SPECTATORS);
            self.server
                .wait_for(sent, CONSOLE_TIMEOUT, probe::count_or_zero)
        } else {
            None
        };
        self.idle_for(Duration::from_secs(1));
        let watcher_game_mode = self
            .watcher_snapshot()
            .and_then(|snapshot| snapshot.game_mode);
        Some(PlayerModeCheck {
            command,
            spectators,
            watcher_game_mode,
        })
    }

    fn panic_when_due(&mut self) {
        let Some(due) = self.panic.next else {
            return;
        };
        if Instant::now() >= due {
            self.panic.lines.extend(scene::panic_hits(
                self.generation,
                self.panic.round,
                self.args.stimulus_pigs,
                self.args.pigs,
            ));
            self.panic.total = self.panic.lines.len();
            self.panic.started = Instant::now();
            self.panic.round += 1;
            self.panic.next = Some(due + self.args.stimulus_period);
        }
        let spread = self
            .args
            .stimulus_period
            .mul_f64(PANIC_SPREAD)
            .as_secs_f64();
        let share = (self.panic.started.elapsed().as_secs_f64() / spread.max(1e-3)).min(1.0);
        let due_lines = (self.panic.total as f64 * share).ceil() as usize;
        let sent = self.panic.total - self.panic.lines.len();
        for _ in sent..due_lines {
            let Some(line) = self.panic.lines.pop_front() else {
                break;
            };
            let _ = self.server.send(&line);
            self.panic.sent += 1;
        }
    }

    /// Serves the stir lines of each game tick that passed since the last served one. The game
    /// time comes from a `time query gametime` that goes out once the one before is answered, so
    /// a slow server gets the same lines a game tick as a fast one.
    fn stir_when_due(&mut self) {
        let Some(stir) = self.stir.stir else {
            return;
        };
        if let Some(sent) = self.stir.query {
            let latest = self
                .server
                .lines_between(sent, Instant::now() + Duration::from_secs(1))
                .iter()
                .rev()
                .find_map(|line| probe::game_time(&line.text));
            if let Some(now) = latest {
                self.stir.query = None;
                let next = self
                    .stir
                    .next_tick
                    .unwrap_or(now)
                    .max(now + 1 - stir::PERIOD);
                for tick in next..=now {
                    for line in stir.lines(tick) {
                        let _ = self.server.send(&line);
                        self.stir.lines_sent += 1;
                    }
                    self.stir.ticks_served += 1;
                }
                self.stir.next_tick = Some(next.max(now + 1));
            } else if sent.elapsed() > STIR_QUERY_TIMEOUT {
                self.stir.query = None;
            }
        }
        if self.stir.query.is_none() {
            self.stir.query = Some(Instant::now());
            let _ = self.server.send("time query gametime");
        }
    }

    fn start_stir(&mut self, phase: Phase) {
        let Some(population) = self.pigs_in else {
            return;
        };
        if !phase.stirred() {
            return;
        }
        let margin = if phase.roams() { ROAM_STIR_MARGIN } else { 1 };
        self.stir.stir = Some(Stir {
            mode: self.args.stir_mode,
            area: population.area,
            margin,
            generation: self.generation,
            count: self.args.pigs,
        });
        self.stir.query = None;
        self.stir.next_tick = None;
    }

    fn stop_stir(&mut self) {
        self.stir.stir = None;
        self.stir.query = None;
        self.stir.next_tick = None;
        self.panic.next = None;
        self.panic.lines.clear();
    }

    fn game_time_now(&mut self) -> Option<i64> {
        let sent = Instant::now();
        self.server.send("time query gametime").ok()?;
        self.server
            .wait_for(sent, CONSOLE_TIMEOUT, probe::game_time)
    }

    /// Waits until `ticks` game ticks passed since `since`, so that a `tick query` answer in the
    /// window holds no tick from before the warm-up.
    fn wait_ticks(&mut self, since: Option<i64>, ticks: i64) {
        let Some(since) = since else {
            return;
        };
        let deadline = Instant::now() + POPULATE_TIMEOUT;
        while Instant::now() < deadline {
            if self.game_time_now().is_none_or(|now| now >= since + ticks) {
                return;
            }
            self.idle_for(Duration::from_secs(2));
        }
    }

    fn idle_for(&mut self, duration: Duration) {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            self.stir_when_due();
            self.panic_when_due();
            self.server.pump();
            std::thread::sleep(Duration::from_millis(20));
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

    /// Puts the pigs of `population` in the world, or removes them for `None`.
    fn populate(&mut self, population: Option<Population>) -> anyhow::Result<()> {
        if self.pigs_in == population {
            return Ok(());
        }
        if self.pigs_in.is_some() {
            self.server.send(scene::KILL_PIGS)?;
            self.wait_count(|count| count == 0)?;
            self.pigs_in = None;
        }
        let Some(population) = population else {
            return Ok(());
        };
        self.generation += 1;
        let lines = scene::summons(
            population.area,
            self.generation,
            self.args.pigs,
            self.args.samples,
            population.ai,
        );
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
        self.pigs_in = Some(population);
        Ok(())
    }

    fn wait_count(&mut self, done: impl Fn(usize) -> bool) -> anyhow::Result<()> {
        let deadline = Instant::now() + POPULATE_TIMEOUT;
        let mut last = None;
        let mut changed = Instant::now();
        while Instant::now() < deadline && changed.elapsed() < STALL_TIMEOUT {
            let count = self.count_pigs();
            if count.is_some_and(&done) {
                return Ok(());
            }
            if count != last {
                last = count;
                changed = Instant::now();
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
        let population = match phase {
            Phase::Idle => None,
            _ if phase.roams() => Some(Population {
                area: ROAM,
                ai: phase.ai(),
            }),
            _ => Some(Population {
                area: PEN,
                ai: phase.ai(),
            }),
        };
        if self.console
            && let Err(error) = self.populate(population)
        {
            return (PhaseReport::skipped(phase, format!("{error:#}")), None);
        }
        let watch = if phase.roams() {
            ROAM.center() + DVec3::Y * HOVER
        } else {
            PEN_WATCH
        };
        if let Some(player) = self.player.clone() {
            let _ = self.server.send(&scene::teleport(&player, watch));
        }
        let pigs_before = self.count_pigs();
        let setup_seconds = round(setup_start.elapsed().as_secs_f64());
        let warmup = match phase {
            Phase::Idle => self.args.idle_warmup,
            Phase::Stimulus => self.args.stimulus_warmup,
            _ => self.args.warmup,
        };
        self.start_stir(phase);
        if phase == Phase::Stimulus {
            self.panic.next = Some(Instant::now());
        }
        let warmup_start = Instant::now();
        let changed_at = if self.console {
            self.game_time_now()
        } else {
            None
        };
        self.idle_for(warmup);
        self.wait_ticks(changed_at, WARMUP_TICKS);
        let warmup = warmup_start.elapsed();
        let (mut report, window) = self.measure(phase);
        self.stop_stir();
        report.pigs_before = pigs_before;
        report.setup_seconds = setup_seconds;
        report.warmup_seconds = round(warmup.as_secs_f64());
        report.external.sprint = self.sprint();
        report.pigs_after = self.count_pigs();
        (report, Some(window))
    }

    /// Sends one probe round, `time query gametime` last, unless the game time answer of the
    /// previous round is still out: a server that answers slower than the rounds go out would
    /// otherwise build a backlog of commands.
    fn probe_round(&mut self, first: bool) {
        if !self.console {
            return;
        }
        if let Some(sent) = self.probe_sent {
            let answered = self
                .server
                .lines_between(sent, Instant::now() + Duration::from_secs(1))
                .iter()
                .any(|line| probe::game_time(&line.text).is_some());
            if !answered {
                self.probe_rounds_skipped += 1;
                return;
            }
        }
        self.probe_sent = Some(Instant::now());
        let samples = if self.pigs_in.is_some() {
            self.args.samples
        } else {
            0
        };
        for sample in 0..samples {
            let line =
                scene::sample_query(self.generation, self.args.pigs, self.args.samples, sample);
            let _ = self.server.send(&line);
        }
        if !first && self.metric == TickMetric::TickQuery {
            let _ = self.server.send("tick query");
        }
        let _ = self.server.send("time query gametime");
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
        self.probe_sent = None;
        self.probe_rounds_skipped = 0;
        let stir_start = self.stir_counts();
        while Instant::now() < until {
            let now = Instant::now();
            if now >= next_probe {
                self.probe_round(first);
                first = false;
                next_probe += self.args.probe_interval;
            }
            self.stir_when_due();
            self.panic_when_due();
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
        self.server.pump();
        let stir_end = self.stir_counts();
        let end_os = os::snapshot();
        let watcher_end = self.watcher_snapshot();
        let load_average_end = os::load_average();
        let until = Instant::now();
        self.idle_for(ANSWER_SLACK);
        let harness: Vec<i32> = self.bot.iter().map(|bot| bot.id() as i32).collect();
        let cpu = os::cpu_between(&start_os, &end_os, self.server.pid(), &harness);
        let contended = cpu.other_cores > self.args.contended_cores;
        let seconds = until.duration_since(from).as_secs_f64().max(1e-9);
        let stir = self.stir.stir.map(|stir| {
            let answers = stir_end.damage_answers - stir_start.damage_answers;
            StirReport {
                mode: stir.mode,
                pacing: "game time",
                ticks_served: stir_end.ticks_served - stir_start.ticks_served,
                lines_sent: stir_end.lines_sent - stir_start.lines_sent,
                panic_lines_sent: stir_end.panic_lines_sent - stir_start.panic_lines_sent,
                damage_answers: answers,
                damage_answers_per_second: round(answers as f64 / seconds),
            }
        });
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
            stir,
            cpu: Some(cpu),
            server_rss_mib_mean: (!rss.is_empty())
                .then(|| round(rss.iter().sum::<f64>() / rss.len() as f64)),
            server_rss_mib_max: rss.iter().copied().reduce(f64::max).map(round),
            load_average_start,
            load_average_end,
            load_average_1m_max: round(load_max.max(load_average_end[0])),
            contended,
            probe_rounds_skipped: self.probe_rounds_skipped,
        };
        (report, Window { from, until })
    }

    fn stir_counts(&self) -> StirCounts {
        StirCounts {
            ticks_served: self.stir.ticks_served,
            lines_sent: self.stir.lines_sent,
            panic_lines_sent: self.panic.sent,
            damage_answers: self.server.damage_answers,
        }
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
