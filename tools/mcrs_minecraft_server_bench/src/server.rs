use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::SocketAddr;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::bail;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
use mcrs_minecraft_protocol::packets::status::serverbound::StatusRequest;
use mcrs_minecraft_protocol::{Bounded, VarInt};
use serde::Serialize;

/// The marker of the line the mcrs server loop logs after each tick, at trace level.
const MCRS_TICK: &str = "mcrs_tick: micros=";
const VANILLA_AVERAGE: &str = "Average time per tick: ";
const VANILLA_PERCENTILES: &str = "Percentiles: P50: ";
/// The window of one vanilla tick query, so the mcrs ticks are cut into windows of the same size.
pub const WINDOW_TICKS: usize = 100;
const LINUX_CLOCK_TICKS_PER_SECOND: f64 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mcrs,
    Vanilla,
}

impl FromStr for Kind {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "mcrs" => Kind::Mcrs,
            "vanilla" => Kind::Vanilla,
            other => bail!("unknown server {other}"),
        })
    }
}

pub struct Server {
    child: Child,
    console: ChildStdin,
    log: JoinHandle<Vec<Line>>,
}

pub struct Line {
    pub at: Instant,
    pub text: String,
}

impl Server {
    /// Starts `command` with its standard error joined to its output, which a thread keeps with
    /// the arrival time of each line.
    pub fn start(command: &[String]) -> anyhow::Result<Self> {
        let Some((program, arguments)) = command.split_first() else {
            bail!("no server command after --");
        };
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(r#"exec "$@" 2>&1"#)
            .arg("sh")
            .arg(program)
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let (Some(console), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            bail!("the server has no console");
        };
        let log = std::thread::spawn(move || read_lines(output));
        Ok(Self {
            child,
            console,
            log,
        })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn exited(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    pub fn command(&mut self, line: &str) -> anyhow::Result<()> {
        writeln!(self.console, "{line}")?;
        Ok(self.console.flush()?)
    }

    /// Asks the server to stop, kills it after `grace`, and returns its log.
    pub fn stop(mut self, kind: Kind, grace: Duration) -> Vec<Line> {
        match kind {
            Kind::Vanilla => {
                let _ = self.command("stop");
            }
            Kind::Mcrs => {
                let _ = Command::new("kill")
                    .arg("-INT")
                    .arg(self.pid().to_string())
                    .status();
            }
        }
        let deadline = Instant::now() + grace;
        while !self.exited() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(200));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.log.join().unwrap_or_default()
    }
}

fn read_lines(output: impl Read) -> Vec<Line> {
    BufReader::new(output)
        .lines()
        .map_while(Result::ok)
        .map(|text| Line {
            at: Instant::now(),
            text,
        })
        .collect()
}

/// Whether the server at `address` answers a status request: both servers answer once their
/// tick loop runs.
pub async fn answers_status(address: SocketAddr) -> bool {
    let request = async {
        let mut io = PacketIo::connect(address).await?;
        io.send_packet(&ServerboundHandshake {
            protocol_version: VarInt(VERSION.protocol_version),
            server_address: Bounded("127.0.0.1"),
            server_port: address.port(),
            intent: Intent::Status,
        })
        .await?;
        io.send_packet(&StatusRequest).await?;
        io.recv_packet::<StatusResponse>().await?;
        anyhow::Ok(())
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(2), request).await,
        Ok(Ok(()))
    )
}

/// The resident memory in bytes and the CPU time in seconds of `root` and its descendants, read
/// from `/proc`; `None` where there is no `/proc`.
pub fn process_tree(root: u32) -> Option<Usage> {
    let mut stats: Vec<ProcStat> = Vec::new();
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if let Some(stat) = proc_stat(pid) {
            stats.push(stat);
        }
    }
    let mut tree = vec![root];
    let mut index = 0;
    while let Some(&parent) = tree.get(index) {
        tree.extend(
            stats
                .iter()
                .filter(|stat| stat.ppid == parent)
                .map(|stat| stat.pid),
        );
        index += 1;
    }
    let mut usage = Usage::default();
    for stat in stats.iter().filter(|stat| tree.contains(&stat.pid)) {
        usage.cpu_seconds += stat.cpu_ticks as f64 / LINUX_CLOCK_TICKS_PER_SECOND;
        usage.rss_bytes += stat.rss_bytes;
    }
    Some(usage)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Usage {
    pub rss_bytes: u64,
    pub cpu_seconds: f64,
}

struct ProcStat {
    pid: u32,
    ppid: u32,
    cpu_ticks: u64,
    rss_bytes: u64,
}

fn proc_stat(pid: u32) -> Option<ProcStat> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name may hold spaces and parentheses; the fields after it do not.
    let fields: Vec<&str> = stat.get(stat.rfind(')')? + 2..)?.split(' ').collect();
    let field = |index: usize| fields.get(index)?.parse::<u64>().ok();
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let rss_kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.trim().trim_end_matches(" kB").parse::<u64>().ok())
        .unwrap_or(0);
    Some(ProcStat {
        pid,
        ppid: u32::try_from(field(1)?).ok()?,
        cpu_ticks: field(11)? + field(12)?,
        rss_bytes: rss_kib * 1024,
    })
}

/// One tick window: the mean and percentiles of its tick times, in milliseconds.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub average: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

/// The tick windows that close inside `from..until`: for vanilla the answers to `/tick query`,
/// for mcrs the logged ticks cut into windows of [`WINDOW_TICKS`].
pub fn windows(kind: Kind, log: &[Line], from: Instant, until: Instant) -> Vec<Window> {
    let inside = log.iter().filter(|line| line.at >= from && line.at < until);
    match kind {
        Kind::Vanilla => vanilla_windows(inside),
        Kind::Mcrs => {
            let ticks: Vec<f64> = inside
                .filter_map(|line| line.text.split_once(MCRS_TICK))
                .filter_map(|(_, micros)| micros.trim().parse::<f64>().ok())
                .map(|micros| micros / 1e3)
                .collect();
            let windows = ticks.as_chunks::<WINDOW_TICKS>().0;
            windows.iter().map(window).collect()
        }
    }
}

/// The mean and the nearest-rank percentiles of `ticks`.
fn window(ticks: &[f64; WINDOW_TICKS]) -> Window {
    let mut sorted = *ticks;
    sorted.sort_by(f64::total_cmp);
    let rank = |quantile: f64| {
        let index = (quantile * WINDOW_TICKS as f64).ceil() as usize;
        sorted.get(index.saturating_sub(1)).copied().unwrap_or(0.0)
    };
    Window {
        average: sorted.iter().sum::<f64>() / WINDOW_TICKS as f64,
        p50: rank(0.50),
        p95: rank(0.95),
        p99: rank(0.99),
    }
}

/// `Average time per tick: 0.1ms (Target: 50.0ms)` and then
/// `Percentiles: P50: 0.1ms P95: 0.1ms P99: 1.6ms. Sample: 100`.
fn vanilla_windows<'a>(lines: impl Iterator<Item = &'a Line>) -> Vec<Window> {
    let mut windows = Vec::new();
    let mut average = None;
    for line in lines {
        if let Some((_, rest)) = line.text.split_once(VANILLA_AVERAGE) {
            average = millis(rest);
        } else if let Some((_, rest)) = line.text.split_once(VANILLA_PERCENTILES) {
            let mut parts = rest.split(" P9").map(millis);
            let (Some(average), Some(Some(p50)), Some(Some(p95)), Some(Some(p99))) =
                (average.take(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            windows.push(Window {
                average,
                p50,
                p95,
                p99,
            });
        }
    }
    windows
}

/// The number before the first `ms`, after an optional `5: ` or `9: ` label.
fn millis(text: &str) -> Option<f64> {
    let (number, _) = text.split_once("ms")?;
    let number = number.rsplit(' ').next()?;
    number.parse().ok()
}
