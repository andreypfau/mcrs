//! Loads the embedded server with bots over TCP and reports how long its ticks take.
//!
//! ```text
//! cargo run --release -p mcrs_minecraft_server --example tick_load -- <bots> <single|parallel> [apart|together] [seconds]
//! ```
//!
//! Every third bot stays in the overworld, the others move to the nether and the end. The bots
//! keep walking from spawn, so the dimensions keep generating, lighting and sending columns.
//! `apart`, the default, gives each bot its own heading, so bots rarely see the same column;
//! `together` walks every bot of a dimension on one heading, so they all see the same columns.
//! The tick report covers the ticks after the first third of the run; the column report covers
//! each bot over the whole run, by the dimension the bot ends up in.

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use bevy_app::{App, PluginGroup};
use bevy_math::DVec3;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::{
    BoundAddress, GameMode, ServerNetworkPlugin, TickThreading, runner,
};
use mcrs_minecraft_engine_world::tick_threading;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::anyhow;
use mcrs_minecraft_protocol::packets::common::serverbound::KeepAlive;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundKeepAlive;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Bounded, Decode, Packet, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundChunkBatchFinished, ClientboundPlayerPosition,
};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundChatCommand, ServerboundChunkBatchReceived,
    ServerboundMovePlayerPos,
};
use mcrs_minecraft_server::MinecraftServerPlugins;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const TICK: Duration = Duration::from_millis(50);
const VIEW_DISTANCE: u8 = 8;
const BLOCKS_PER_TICK: f64 = 0.6;
const OTHER_DIMENSIONS: [&str; 2] = ["minecraft:the_nether", "minecraft:the_end"];

/// How the bots of a dimension spread out from spawn.
#[derive(Clone, Copy, Debug)]
enum Spread {
    Apart,
    Together,
}

impl FromStr for Spread {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        match name {
            "apart" => Ok(Self::Apart),
            "together" => Ok(Self::Together),
            other => anyhow::bail!("unknown spread {other:?}, expected apart or together"),
        }
    }
}

struct TickTimes {
    tick: Duration,
    drain: Duration,
    /// The wakes the dimensions sent until the next tick.
    wakes: usize,
    /// The drains that ran a dimension until the next tick.
    drains: usize,
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let bots: usize = args.get(1).map_or(Ok(30), |count| count.parse())?;
    let threading = args
        .get(2)
        .map_or(Ok(TickThreading::Parallel), |name| name.parse())?;
    let spread: Spread = args.get(3).map_or(Ok(Spread::Apart), |name| name.parse())?;
    let seconds: u64 = args.get(4).map_or(Ok(40), |count| count.parse())?;

    let mut plugins = MinecraftServerPlugins::default();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.core.tick_threading = threading;
    plugins.network = ServerNetworkPlugin::embedded();
    plugins.network.announce_on_lan = false;
    plugins.worlds.default_op_level = 2;
    plugins.worlds.default_game_mode = GameMode::Creative;
    let mut app = App::new();
    app.add_plugins(plugins.build());
    let address = app.world().resource::<BoundAddress>().0;

    let stop = Arc::new(AtomicBool::new(false));
    let (times_send, times_recv) = mpsc::channel();
    let server_stop = stop.clone();
    let server = mcrs_minecraft_engine_server::spawn_server_thread(app, move |app| {
        times_send.send(run_timed(app, &server_stop)).ok();
    })?;

    let started = Instant::now();
    let counts = Arc::new(Counts::new(bots, started));
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        for index in 0..bots {
            let counts = counts.clone();
            tokio::spawn(async move {
                if let Err(error) = bot(address, index, spread, &counts).await {
                    let after = started.elapsed().as_secs_f64();
                    eprintln!("bot{index} stopped after {after:.1} s: {error:#}");
                }
            });
        }
        tokio::time::sleep(Duration::from_secs(seconds)).await;
    });
    stop.store(true, Ordering::Relaxed);
    let times = times_recv.recv()?;
    server
        .join()
        .map_err(|_| anyhow::anyhow!("the server thread panicked"))?;
    drop(runtime);

    let measured = times.get(times.len() / 3..).unwrap_or_default();
    println!(
        "{threading:?}, {spread:?}: {} of {bots} bots joined, {} columns received, {} ticks measured",
        counts.joined.load(Ordering::Relaxed),
        counts.columns.load(Ordering::Relaxed),
        measured.len()
    );
    report("tick", measured.iter().map(|times| times.tick));
    report("drain", measured.iter().map(|times| times.drain));
    report_wakes(measured);
    report_bots(&counts);
    Ok(())
}

/// `run_server_loop`, timing the tick and the column drains until the next tick.
fn run_timed(mut app: App, stop: &AtomicBool) -> Vec<TickTimes> {
    app.finish();
    app.cleanup();
    let mut times = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        let start = Instant::now();
        tick_threading::update(&mut app);
        runner::between_ticks(&mut app);
        let tick = start.elapsed();
        // `runner::drain_until`, without the time spent waiting.
        let deadline = start + TICK;
        let mut drain = Duration::ZERO;
        let mut wakes = 0;
        let mut drains = 0;
        loop {
            let drained = Instant::now();
            let taken = runner::drain_woken(&mut app);
            drain += drained.elapsed();
            wakes += taken.wakes;
            drains += taken.dimensions;
            if !runner::wait_for_wake(&mut app, deadline) {
                break;
            }
        }
        runner::sleep_until(deadline);
        times.push(TickTimes {
            tick,
            drain,
            wakes,
            drains,
        });
    }
    times
}

/// The wakes and the dimension drains a tick, and what a drain costs on average.
fn report_wakes(measured: &[TickTimes]) {
    let ticks = measured.len().max(1) as f64;
    let wakes: usize = measured.iter().map(|times| times.wakes).sum();
    let drains: usize = measured.iter().map(|times| times.drains).sum();
    let drain: Duration = measured.iter().map(|times| times.drain).sum();
    println!(
        "wakes: {:.1} a tick, dimension drains: {:.1} a tick, {:.1} us a drain",
        wakes as f64 / ticks,
        drains as f64 / ticks,
        drain.as_secs_f64() * 1e6 / drains.max(1) as f64,
    );
}

fn report(name: &str, durations: impl Iterator<Item = Duration>) {
    let mut sorted: Vec<Duration> = durations.collect();
    sorted.sort_unstable();
    let Some(max) = sorted.last() else {
        println!("{name}: no ticks");
        return;
    };
    let at = |quantile: f64| {
        let index = ((sorted.len() - 1) as f64 * quantile).round() as usize;
        sorted.get(index).copied().unwrap_or_default()
    };
    let mean = sorted.iter().sum::<Duration>() / sorted.len() as u32;
    println!(
        "{name}: mean {:.3} ms, p50 {:.3}, p95 {:.3}, p99 {:.3}, max {:.3}",
        mean.as_secs_f64() * 1e3,
        at(0.5).as_secs_f64() * 1e3,
        at(0.95).as_secs_f64() * 1e3,
        at(0.99).as_secs_f64() * 1e3,
        max.as_secs_f64() * 1e3,
    );
}

struct Counts {
    started: Instant,
    joined: AtomicUsize,
    columns: AtomicUsize,
    bots: Vec<BotCounts>,
}

impl Counts {
    fn new(bots: usize, started: Instant) -> Self {
        Self {
            started,
            joined: AtomicUsize::new(0),
            columns: AtomicUsize::new(0),
            bots: (0..bots).map(|_| BotCounts::default()).collect(),
        }
    }

    fn received(&self, index: usize, columns: usize) {
        self.columns.fetch_add(columns, Ordering::Relaxed);
        let Some(bot) = self.bots.get(index) else {
            return;
        };
        let millis = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        bot.columns.fetch_add(columns, Ordering::Relaxed);
        bot.first_millis.fetch_min(millis, Ordering::Relaxed);
        bot.last_millis.fetch_max(millis, Ordering::Relaxed);
    }
}

/// The columns one bot received, and when the first and the last of them arrived.
struct BotCounts {
    columns: AtomicUsize,
    first_millis: AtomicU64,
    last_millis: AtomicU64,
}

impl Default for BotCounts {
    fn default() -> Self {
        Self {
            columns: AtomicUsize::new(0),
            first_millis: AtomicU64::new(u64::MAX),
            last_millis: AtomicU64::new(0),
        }
    }
}

/// The name of the dimension the bot with `index` ends up in.
fn destination(index: usize) -> &'static str {
    OTHER_DIMENSIONS
        .get(index % 3)
        .copied()
        .unwrap_or("minecraft:overworld")
}

/// The columns per bot, by the dimension each bot ends up in, and how soon the bots got their
/// first column.
fn report_bots(counts: &Counts) {
    let mut dimensions: Vec<&str> = (0..counts.bots.len()).map(destination).collect();
    dimensions.sort_unstable();
    dimensions.dedup();
    for dimension in dimensions {
        let mut columns: Vec<usize> = (0..counts.bots.len())
            .filter(|index| destination(*index) == dimension)
            .filter_map(|index| counts.bots.get(index))
            .map(|bot| bot.columns.load(Ordering::Relaxed))
            .collect();
        columns.sort_unstable();
        let median = columns.get(columns.len() / 2).copied().unwrap_or(0);
        println!(
            "{dimension}: {} bots, columns a bot min {}, p50 {median}, max {}",
            columns.len(),
            columns.first().copied().unwrap_or(0),
            columns.last().copied().unwrap_or(0),
        );
    }
    let starved = counts
        .bots
        .iter()
        .filter(|bot| bot.columns.load(Ordering::Relaxed) == 0)
        .count();
    let mut firsts: Vec<u64> = counts
        .bots
        .iter()
        .map(|bot| bot.first_millis.load(Ordering::Relaxed))
        .filter(|millis| *millis != u64::MAX)
        .collect();
    firsts.sort_unstable();
    let mut lasts: Vec<u64> = counts
        .bots
        .iter()
        .filter(|bot| bot.columns.load(Ordering::Relaxed) > 0)
        .map(|bot| bot.last_millis.load(Ordering::Relaxed))
        .collect();
    lasts.sort_unstable();
    let median_seconds = |sorted: &[u64]| {
        let median = sorted.get(sorted.len() / 2).copied().unwrap_or(0);
        median as f64 / 1e3
    };
    println!(
        "bots with no column: {starved}, first column after p50 {:.1} s (max {:.1} s), last column at p50 {:.1} s",
        median_seconds(&firsts),
        firsts.last().copied().unwrap_or(0) as f64 / 1e3,
        median_seconds(&lasts),
    );
}

async fn bot(
    address: std::net::SocketAddr,
    index: usize,
    spread: Spread,
    counts: &Counts,
) -> anyhow::Result<()> {
    let io = PacketIo::connect(address).await?;
    let Joined { mut connection, .. } = join(
        io,
        Join {
            server_address: address.ip().to_string(),
            server_port: address.port(),
            remote_addr: address,
            username: format!("bot{index}"),
            profile_id: Uuid::nil(),
            protocol_version: VERSION.protocol_version,
            view_distance: VIEW_DISTANCE,
            holds_vanilla_pack: false,
        },
    )
    .await?;
    counts.joined.fetch_add(1, Ordering::Relaxed);

    let heading = match spread {
        Spread::Apart => index as f64 * 2.399_963,
        Spread::Together => 0.7,
    };
    let step = DVec3::new(heading.cos(), 0.0, heading.sin()) * BLOCKS_PER_TICK;
    let mut destination = OTHER_DIMENSIONS.get(index % 3);
    let mut position: Option<DVec3> = None;
    let mut ticks = tokio::time::interval(TICK);
    loop {
        ticks.tick().await;
        while let Some(packet) = connection.try_recv()? {
            let body = &mut &packet.payload[..];
            if packet.id == ClientboundPlayerPosition::ID {
                let teleport = ClientboundPlayerPosition::decode(body)?;
                position = Some(teleport.position);
                connection.write_packet(&ServerboundAcceptTeleportation {
                    teleport_id: teleport.teleport_id,
                    position: teleport.position.into(),
                    look: teleport.look,
                });
            } else if packet.id == ClientboundChunkBatchFinished::ID {
                let finished = ClientboundChunkBatchFinished::decode(body)?;
                let columns = usize::try_from(finished.batch_size.0).unwrap_or(0);
                counts.received(index, columns);
                connection.write_packet(&ServerboundChunkBatchReceived {
                    desired_chunks_per_tick: 64.0,
                });
            } else if packet.id == ClientboundKeepAlive::ID {
                let keep_alive = ClientboundKeepAlive::decode(body)?;
                connection.write_packet(&ServerboundKeepAlive(KeepAlive {
                    payload: keep_alive.0.payload,
                }));
            }
        }
        if let Some(at) = position.as_mut() {
            if let Some(dimension) = destination.take() {
                let command = format!("execute in {dimension} run tp @s 0 100 0");
                connection.write_packet(&ServerboundChatCommand {
                    command: Bounded(&command),
                });
                position = None;
            } else {
                *at += step;
                connection.write_packet(&ServerboundMovePlayerPos {
                    position: (*at).into(),
                    flags: MoveFlags::new(),
                });
            }
        }
        connection.flush()?;
    }
}
