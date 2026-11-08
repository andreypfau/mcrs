//! Loads one dedicated server, mcrs or vanilla, with protocol bots and writes one JSON report.
//!
//! ```text
//! mcrs_minecraft_server_bench <mcrs|vanilla> <bots> <walk|explore|idle|mixed> <out.json> \
//!     [--warmup 60] [--measure 120] [--explore-speed 0.8] -- <server command>
//! ```
//!
//! See `README.md` for the method.

mod bot;
mod server;

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use mcrs_voxel::ColumnPos;
use serde::Serialize;

use bot::{Behaviour, BotReport, Plan};
use server::{Kind, Server, Usage, Window};

/// The port `run.sh` gives both servers.
const PORT: u16 = 25565;
const JOIN_INTERVAL: Duration = Duration::from_secs(1);
const READY_TIMEOUT: Duration = Duration::from_secs(600);
const STOP_GRACE: Duration = Duration::from_secs(60);
const QUERY_INTERVAL: Duration = Duration::from_secs(5);
/// Vanilla's game rules set to what mcrs does not do yet; `respawn_radius 0` puts every bot on
/// one spawn point, as mcrs does.
const VANILLA_RULES: [&str; 4] = [
    "gamerule locator_bar false",
    "gamerule spawn_mobs false",
    "gamerule spawn_monsters false",
    "gamerule respawn_radius 0",
];

struct Args {
    kind: Kind,
    bots: usize,
    behaviour: Behaviour,
    out: String,
    warmup: Duration,
    measure: Duration,
    explore_speed: f64,
    command: Vec<String>,
}

fn parse_args() -> anyhow::Result<Args> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let split = all
        .iter()
        .position(|arg| arg == "--")
        .context("no -- before the server command")?;
    let (own, command) = all.split_at(split);
    let [kind, bots, behaviour, out, options @ ..] = own else {
        bail!("usage: <mcrs|vanilla> <bots> <behaviour> <out.json> [options] -- <command>");
    };
    let mut args = Args {
        kind: kind.parse()?,
        bots: bots.parse()?,
        behaviour: behaviour.parse()?,
        out: out.clone(),
        warmup: Duration::from_secs(60),
        measure: Duration::from_secs(120),
        explore_speed: 0.8,
        command: command.iter().skip(1).cloned().collect(),
    };
    for pair in options.chunks(2) {
        let [name, value] = pair else {
            bail!("option {pair:?} has no value");
        };
        match name.as_str() {
            "--warmup" => args.warmup = Duration::from_secs(value.parse()?),
            "--measure" => args.measure = Duration::from_secs(value.parse()?),
            "--explore-speed" => args.explore_speed = value.parse()?,
            other => bail!("unknown option {other}"),
        }
    }
    Ok(args)
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, PORT));
    let started = Instant::now();
    let mut server = Server::start(&args.command)?;
    let ready = loop {
        if server.exited() {
            bail!("the server exited before it answered a status request");
        }
        if runtime.block_on(server::answers_status(address)) {
            break started.elapsed();
        }
        if started.elapsed() > READY_TIMEOUT {
            bail!("the server did not answer a status request in {READY_TIMEOUT:?}");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if args.kind == Kind::Vanilla {
        for rule in VANILLA_RULES {
            server.command(rule)?;
        }
    }

    let joins_end = tokio::time::Instant::now() + JOIN_INTERVAL * args.bots as u32;
    let measure_from = joins_end + args.warmup;
    let measure_until = measure_from + args.measure;
    let mut bots = Vec::new();
    for index in 0..args.bots {
        let plan = Plan {
            address,
            index,
            behaviour: args.behaviour,
            explore_blocks_per_second: args.explore_speed,
            measure_from,
            measure_until,
        };
        bots.push(runtime.spawn(bot::run(plan)));
        std::thread::sleep(JOIN_INTERVAL);
    }

    let measure_from = measure_from.into_std();
    let measure_until = measure_until.into_std();
    let mut usage: Vec<Usage> = Vec::new();
    let mut last_query = Instant::now();
    while Instant::now() < measure_until + Duration::from_secs(1) {
        let now = Instant::now();
        if now >= measure_from
            && let Some(sample) = server::process_tree(server.pid())
        {
            usage.push(sample);
        }
        if args.kind == Kind::Vanilla && now.duration_since(last_query) >= QUERY_INTERVAL {
            server.command("tick query")?;
            last_query = now;
        }
        if server.exited() {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    let exited_early = server.exited();

    let mut reports = Vec::new();
    let mut failures = Vec::new();
    for (index, handle) in bots.into_iter().enumerate() {
        match runtime.block_on(handle) {
            Ok(Ok(report)) => reports.push(report),
            Ok(Err(error)) => failures.push(format!("bot{index}: {error:#}")),
            Err(error) => failures.push(format!("bot{index}: {error}")),
        }
    }
    let log = server.stop(args.kind, STOP_GRACE);
    let text: Vec<&str> = log.iter().map(|line| line.text.as_str()).collect();
    std::fs::write(format!("{}.log", args.out), text.join("\n"))?;
    let windows = server::windows(args.kind, &log, measure_from, measure_until);
    let report = Report {
        server: args.kind,
        bots: args.bots,
        behaviour: args.behaviour,
        explore_blocks_per_second: args.explore_speed,
        warmup_seconds: args.warmup.as_secs(),
        measure_seconds: args.measure.as_secs(),
        start_seconds: round(ready.as_secs_f64()),
        exited_early,
        mspt: mspt(&windows),
        usage: usage_summary(&usage, args.measure),
        work: work(&reports, args.measure),
        bots_failed: failures,
        log_moved_wrongly: count(&log, "moved wrongly"),
        log_moved_too_quickly: count(&log, "moved too quickly"),
        command: args.command,
    };
    let text = serde_json::to_string_pretty(&report)?;
    std::fs::write(&args.out, &text)?;
    println!("{text}");
    Ok(())
}

#[derive(Serialize)]
struct Report {
    server: Kind,
    bots: usize,
    behaviour: Behaviour,
    explore_blocks_per_second: f64,
    warmup_seconds: u64,
    measure_seconds: u64,
    start_seconds: f64,
    exited_early: bool,
    mspt: Option<Mspt>,
    usage: Option<UsageSummary>,
    work: Work,
    bots_failed: Vec<String>,
    log_moved_wrongly: usize,
    log_moved_too_quickly: usize,
    command: Vec<String>,
}

#[derive(Serialize)]
struct Mspt {
    windows: usize,
    average: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    worst_window_p99: f64,
    tps: f64,
}

#[derive(Serialize)]
struct UsageSummary {
    rss_mib_mean: f64,
    rss_mib_max: f64,
    cpu_percent: f64,
}

#[derive(Serialize)]
struct Work {
    bots_measured: usize,
    chunks_per_second: f64,
    first_time_chunks_per_second: f64,
    resent: u64,
    pulled_back: u64,
    moves: u64,
    waits_for_chunk: u64,
}

fn count(log: &[server::Line], needle: &str) -> usize {
    log.iter().filter(|line| line.text.contains(needle)).count()
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// The means of the windows' averages and percentiles, the highest window P99, and TPS as 20
/// while the average fits in a tick, else 1000 / average.
fn mspt(windows: &[Window]) -> Option<Mspt> {
    if windows.is_empty() {
        return None;
    }
    let mean = |pick: fn(&Window) -> f64| {
        round(windows.iter().map(pick).sum::<f64>() / windows.len() as f64)
    };
    let average = mean(|window| window.average);
    Some(Mspt {
        windows: windows.len(),
        average,
        p50: mean(|window| window.p50),
        p95: mean(|window| window.p95),
        p99: mean(|window| window.p99),
        worst_window_p99: windows.iter().map(|window| window.p99).fold(0.0, f64::max),
        tps: round(if average <= 50.0 {
            20.0
        } else {
            1000.0 / average
        }),
    })
}

fn usage_summary(usage: &[Usage], measure: Duration) -> Option<UsageSummary> {
    let (first, last) = (usage.first()?, usage.last()?);
    let mib = |bytes: u64| round(bytes as f64 / (1024.0 * 1024.0));
    let rss_mean = usage.iter().map(|sample| sample.rss_bytes).sum::<u64>() / usage.len() as u64;
    let rss_max = usage.iter().map(|sample| sample.rss_bytes).max()?;
    Some(UsageSummary {
        rss_mib_mean: mib(rss_mean),
        rss_mib_max: mib(rss_max),
        cpu_percent: round((last.cpu_seconds - first.cpu_seconds) / measure.as_secs_f64() * 100.0),
    })
}

/// The equal-work checks: columns a second, columns no bot had before the window a second,
/// columns resent while held, pull-backs, and the ticks a bot waited for its own column.
fn work(reports: &[BotReport], measure: Duration) -> Work {
    let seconds = measure.as_secs_f64();
    let sum = |pick: fn(&BotReport) -> u64| reports.iter().map(pick).sum::<u64>();
    let before: HashSet<ColumnPos> = reports
        .iter()
        .flat_map(|report| report.held_before.iter().copied())
        .collect();
    let first_time: HashSet<&ColumnPos> = reports
        .iter()
        .flat_map(|report| report.received_during.iter())
        .filter(|column| !before.contains(column))
        .collect();
    Work {
        bots_measured: reports.len(),
        chunks_per_second: round(sum(|report| report.columns_during) as f64 / seconds),
        first_time_chunks_per_second: round(first_time.len() as f64 / seconds),
        resent: sum(|report| report.resent_during),
        pulled_back: sum(|report| report.pulled_back_during),
        moves: sum(|report| report.moves_during),
        waits_for_chunk: sum(|report| report.waits_during),
    }
}
