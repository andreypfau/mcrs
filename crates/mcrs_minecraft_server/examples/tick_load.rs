//! Loads the embedded server with bots over TCP and reports how long its ticks take.
//!
//! ```text
//! cargo run --release -p mcrs_minecraft_server --example tick_load -- <bots> <single|parallel> [apart|together] [seconds] [dimensions]
//! ```
//!
//! `single` ticks every world inline on the server thread, `parallel` gives each world a thread.
//!
//! ```text
//! ```
//!
//! The bots spread over the dimensions in turn: bot 0 in the overworld, bot 1 in the nether,
//! bot 2 in the end, and so on. The bots
//! keep walking from spawn, so the dimensions keep generating, lighting and sending columns.
//! `apart`, the default, gives each bot its own heading, so bots rarely see the same column;
//! `together` walks every bot of a dimension on one heading, so they all see the same columns.
//! Every bot moves to the same point first, so `together` is a crowd: each bot sees every other
//! bot of its dimension, and all of them keep producing entity moves. `dimensions` (1 to 3, 3 by
//! default) is the number of dimensions the bots spread over; 1 puts the whole crowd in the
//! overworld.
//! The tick report covers the ticks after the first third of the run; the column report covers
//! each bot over the whole run, by the dimension the bot ends up in.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use bevy_app::{App, PluginGroup};
use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::{
    BoundAddress, GameMode, Placement, ServerNetworkPlugin, runner,
};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::metrics::{BridgeTelemetry, PacketKinds};
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
const DIMENSIONS: [&str; 3] = [
    "minecraft:overworld",
    "minecraft:the_nether",
    "minecraft:the_end",
];

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
    between_ticks: Duration,
    drain: Duration,
    traffic: Traffic,
    /// The bytes appended to each connection during the tick.
    session_bytes: Vec<u64>,
}

/// What the bridge handed to the connections during one tick and the drains after it.
#[derive(Clone, Copy, Default)]
struct Traffic {
    messages: u64,
    encode_calls: u64,
    shared_appends: u64,
    shared_bytes: u64,
    shared_frames: u64,
    framed_appends: u64,
    packets: PacketKinds,
}

impl Traffic {
    fn of(telemetry: &BridgeTelemetry) -> Self {
        Self {
            messages: telemetry.outbound_messages_consumed_total,
            encode_calls: telemetry.encode_calls_total,
            shared_appends: telemetry.shared_appends_total,
            shared_bytes: telemetry.shared_bytes_total,
            shared_frames: telemetry.shared_frames_total,
            framed_appends: telemetry.framed_appends_total,
            packets: telemetry.packets,
        }
    }

    fn since(self, earlier: Self) -> Self {
        Self {
            messages: self.messages.saturating_sub(earlier.messages),
            encode_calls: self.encode_calls.saturating_sub(earlier.encode_calls),
            shared_appends: self.shared_appends.saturating_sub(earlier.shared_appends),
            shared_bytes: self.shared_bytes.saturating_sub(earlier.shared_bytes),
            shared_frames: self.shared_frames.saturating_sub(earlier.shared_frames),
            framed_appends: self.framed_appends.saturating_sub(earlier.framed_appends),
            packets: self.packets.since(earlier.packets),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let bots: usize = args.get(1).map_or(Ok(30), |count| count.parse())?;
    let worlds = match args.get(2).map(String::as_str) {
        None | Some("parallel") => Placement::Thread,
        Some("single") => Placement::Inline,
        Some(other) => anyhow::bail!("unknown mode {other:?}, expected single or parallel"),
    };
    let spread: Spread = args.get(3).map_or(Ok(Spread::Apart), |name| name.parse())?;
    let seconds: u64 = args.get(4).map_or(Ok(40), |count| count.parse())?;
    let dimensions: usize = args.get(5).map_or(Ok(3), |count| count.parse())?;
    anyhow::ensure!(
        (1..=DIMENSIONS.len()).contains(&dimensions),
        "dimensions must be 1 to {}",
        DIMENSIONS.len()
    );

    let mut plugins = MinecraftServerPlugins::default();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.core.worlds = worlds;
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
                if let Err(error) = bot(address, index, spread, dimensions, &counts).await {
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
        "{worlds:?}, {spread:?}: {} of {bots} bots joined, {} columns received, {} ticks measured",
        counts.joined.load(Ordering::Relaxed),
        counts.columns.load(Ordering::Relaxed),
        measured.len()
    );
    report("tick", measured.iter().map(|times| times.tick));
    report(
        "between ticks",
        measured.iter().map(|times| times.between_ticks),
    );
    report("drain", measured.iter().map(|times| times.drain));
    report_traffic(measured);
    report_bots(&counts, dimensions);
    Ok(())
}

/// `run_server_loop`, timing the tick and the hand-over to the sockets after it.
fn run_timed(mut app: App, stop: &AtomicBool) -> Vec<TickTimes> {
    app.finish();
    app.cleanup();
    let mut times = Vec::new();
    let mut sent: HashMap<Entity, u64> = HashMap::new();
    while !stop.load(Ordering::Relaxed) {
        let before = Traffic::of(app.world().resource::<BridgeTelemetry>());
        let start = Instant::now();
        app.update();
        let between = Instant::now();
        runner::between_ticks(&mut app);
        let between_ticks = between.elapsed();
        let tick = start.elapsed();
        // `runner::drain_until`, without the time spent waiting.
        let drained = Instant::now();
        runner::drain(&mut app);
        let drain = drained.elapsed();
        runner::drain_until(&mut app, start + TICK);
        let traffic = Traffic::of(app.world().resource::<BridgeTelemetry>()).since(before);
        let session_bytes = session_bytes(&mut app, &mut sent);
        times.push(TickTimes {
            tick,
            between_ticks,
            drain,
            traffic,
            session_bytes,
        });
    }
    times
}

/// The bytes each connection was handed since the last call.
fn session_bytes(app: &mut App, sent: &mut HashMap<Entity, u64>) -> Vec<u64> {
    let mut query = app.world_mut().query::<(Entity, &ServerSideConnection)>();
    let mut deltas = Vec::new();
    for (entity, connection) in query.iter(app.world()) {
        let total = connection.raw.appended_bytes();
        let last = sent.insert(entity, total).unwrap_or(0);
        deltas.push(total.saturating_sub(last));
    }
    deltas
}

/// The packets and bytes the bridge handed to the connections, per tick and per connection.
fn report_traffic(measured: &[TickTimes]) {
    let ticks = measured.len().max(1) as f64;
    let mut total = Traffic::default();
    for times in measured {
        let traffic = &times.traffic;
        total.messages += traffic.messages;
        total.encode_calls += traffic.encode_calls;
        total.shared_appends += traffic.shared_appends;
        total.shared_bytes += traffic.shared_bytes;
        total.shared_frames += traffic.shared_frames;
        total.framed_appends += traffic.framed_appends;
        total.packets.entity_move += traffic.packets.entity_move;
        total.packets.spawn += traffic.packets.spawn;
        total.packets.remove += traffic.packets.remove;
        total.packets.player_info += traffic.packets.player_info;
        total.packets.chunk += traffic.packets.chunk;
        total.packets.other += traffic.packets.other;
    }
    let a_tick = |count: u64| count as f64 / ticks;
    println!(
        "messages: {:.1} a tick, encode calls: {:.1} a tick, shared appends: {:.1} a tick ({:.1} KB a tick shared)",
        a_tick(total.messages),
        a_tick(total.encode_calls),
        a_tick(total.shared_appends),
        a_tick(total.shared_bytes) / 1e3,
    );
    println!(
        "shared frames made: {:.1} a tick, appends of a made frame: {:.1} a tick",
        a_tick(total.shared_frames),
        a_tick(total.framed_appends),
    );
    println!(
        "packets a tick: move {:.1}, spawn {:.1}, remove {:.1}, player info {:.1}, chunk {:.1}, other {:.1}",
        a_tick(total.packets.entity_move),
        a_tick(total.packets.spawn),
        a_tick(total.packets.remove),
        a_tick(total.packets.player_info),
        a_tick(total.packets.chunk),
        a_tick(total.packets.other),
    );
    let mut bytes: Vec<u64> = measured
        .iter()
        .flat_map(|times| times.session_bytes.iter().copied())
        .collect();
    bytes.sort_unstable();
    let Some(max) = bytes.last() else {
        println!("bytes a session a tick: no sessions");
        return;
    };
    let mean = bytes.iter().sum::<u64>() as f64 / bytes.len() as f64;
    let p95_index = ((bytes.len() - 1) as f64 * 0.95).round() as usize;
    println!(
        "bytes a session a tick: mean {mean:.0}, p95 {}, max {max}",
        bytes.get(p95_index).copied().unwrap_or(0),
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
fn destination(index: usize, dimensions: usize) -> &'static str {
    DIMENSIONS
        .get(index % dimensions)
        .copied()
        .unwrap_or("minecraft:overworld")
}

/// The columns per bot, by the dimension each bot ends up in, and how soon the bots got their
/// first column.
fn report_bots(counts: &Counts, dimensions: usize) {
    let mut names: Vec<&str> = (0..counts.bots.len())
        .map(|index| destination(index, dimensions))
        .collect();
    names.sort_unstable();
    names.dedup();
    for dimension in names {
        let mut columns: Vec<usize> = (0..counts.bots.len())
            .filter(|index| destination(*index, dimensions) == dimension)
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
    dimensions: usize,
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
    let mut destination = Some(destination(index, dimensions));
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
