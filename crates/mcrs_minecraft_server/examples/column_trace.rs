//! Flies one bot in a straight line over the embedded server and prints what the server sent
//! for each column: whole columns, light updates, block updates and forgets, each with the
//! tick and the distance from the bot. It counts the light updates that change no light row of
//! a column the bot holds, which a client would remesh for nothing.
//!
//! ```text
//! cargo run --release -p mcrs_minecraft_server --example column_trace -- [seconds] [blocks_per_tick] [simulation|-] [view]
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bevy_app::{App, Plugin, PluginGroup};
use bevy_math::DVec3;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_engine_server::{BoundAddress, GameMode, ServerNetworkPlugin, runner};
use mcrs_minecraft_engine_world::world::lifecycle::simulation::SimulationDistance;
use mcrs_minecraft_engine_world::world::sub_app::AddDimensionPlugins;
use mcrs_minecraft_network::ReceivedPacket;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::anyhow;
use mcrs_minecraft_protocol::packets::common::serverbound::KeepAlive;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundKeepAlive;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::chunk::LightData;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundBlockUpdate, ClientboundChunkBatchFinished, ClientboundForgetLevelChunk,
    ClientboundLevelChunkWithLight, ClientboundLightUpdate, ClientboundPlayerPosition,
    ClientboundSectionBlocksUpdate,
};
use mcrs_minecraft_protocol_game::light_codec::{RowLight, unpack_light_data};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundChunkBatchReceived, ServerboundMovePlayerPos,
};
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_voxel::ColumnPos;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");
const TICK: Duration = Duration::from_millis(50);
/// The ticks the bot stands at spawn before it flies, so that the first view is sent whole.
const WARMUP_TICKS: u32 = 200;
/// The light rows of an overworld column: its 24 sections and one padding row at each end.
const OVERWORLD_ROWS: usize = 26;
const ROW_BYTES: usize = 2048;

#[derive(Clone)]
struct Simulation(SimulationDistance);

impl Plugin for Simulation {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Column,
    Light,
    Block,
    Forget,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Column => "column",
            Self::Light => "light",
            Self::Block => "block",
            Self::Forget => "forget",
        };
        f.write_str(name)
    }
}

struct Event {
    tick: u32,
    kind: Kind,
    distance: i32,
}

/// What the bot received, by column, and the light rows of every column it holds.
#[derive(Default)]
struct Trace {
    events: BTreeMap<ColumnPos, Vec<Event>>,
    held: BTreeMap<ColumnPos, Vec<[u8; ROW_BYTES]>>,
    light_updates: usize,
    light_updates_changing_nothing: usize,
}

impl Trace {
    fn push(&mut self, column: ColumnPos, kind: Kind, tick: u32, bot: ColumnPos) {
        let distance = (column.x - bot.x).abs().max((column.z - bot.z).abs());
        self.events.entry(column).or_default().push(Event {
            tick,
            kind,
            distance,
        });
    }

    fn hold(&mut self, column: ColumnPos, light: &LightData<'_>) -> anyhow::Result<()> {
        let light = unpack_light_data(light, OVERWORLD_ROWS)?;
        let rows = light
            .sky
            .iter()
            .chain(&light.block)
            .map(|row| match row {
                RowLight::Filled(chunk) => chunk.0,
                RowLight::Unchanged | RowLight::Empty => [0; ROW_BYTES],
            })
            .collect();
        self.held.insert(column, rows);
        Ok(())
    }

    fn relight(&mut self, column: ColumnPos, light: &LightData<'_>) -> anyhow::Result<()> {
        let update = unpack_light_data(light, OVERWORLD_ROWS)?;
        let Some(rows) = self.held.get_mut(&column) else {
            return Ok(());
        };
        let mut changed = false;
        for (row, next) in rows.iter_mut().zip(update.sky.iter().chain(&update.block)) {
            let next = match next {
                RowLight::Unchanged => continue,
                RowLight::Empty => [0; ROW_BYTES],
                RowLight::Filled(chunk) => chunk.0,
            };
            changed |= *row != next;
            *row = next;
        }
        self.light_updates += 1;
        if !changed {
            self.light_updates_changing_nothing += 1;
        }
        Ok(())
    }

    fn record(&mut self, packet: &ReceivedPacket, tick: u32, bot: ColumnPos) -> anyhow::Result<()> {
        let body = &mut &packet.payload[..];
        if packet.id == ClientboundLevelChunkWithLight::ID {
            let chunk = ClientboundLevelChunkWithLight::decode(body)?;
            self.push(chunk.pos, Kind::Column, tick, bot);
            self.hold(chunk.pos, &chunk.light_data)?;
        } else if packet.id == ClientboundLightUpdate::ID {
            let update = ClientboundLightUpdate::decode(body)?;
            let column = ColumnPos::new(update.x.0, update.z.0);
            self.push(column, Kind::Light, tick, bot);
            self.relight(column, &update.light_data)?;
        } else if packet.id == ClientboundForgetLevelChunk::ID {
            let forget = ClientboundForgetLevelChunk::decode(body)?;
            self.push(ColumnPos::new(forget.x, forget.z), Kind::Forget, tick, bot);
        } else if packet.id == ClientboundBlockUpdate::ID {
            let update = ClientboundBlockUpdate::decode(body)?;
            let column = ColumnPos::new(update.block_pos.x >> 4, update.block_pos.z >> 4);
            self.push(column, Kind::Block, tick, bot);
        } else if packet.id == ClientboundSectionBlocksUpdate::ID {
            let update = ClientboundSectionBlocksUpdate::decode(body)?;
            let column = ColumnPos::new(update.chunk_pos.x, update.chunk_pos.z);
            self.push(column, Kind::Block, tick, bot);
        }
        Ok(())
    }

    fn report(&self) {
        let mut sequences: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_distance: BTreeMap<(Kind, i32), usize> = BTreeMap::new();
        let mut sent_again = 0;
        for events in self.events.values() {
            let mut kinds: Vec<String> =
                events.iter().map(|event| event.kind.to_string()).collect();
            kinds.dedup();
            *sequences.entry(kinds.join(">")).or_default() += 1;
            let columns = events.iter().filter(|event| event.kind == Kind::Column);
            if columns.count() > 1 {
                sent_again += 1;
            }
            let flying = events.iter().filter(|event| event.tick > WARMUP_TICKS);
            for event in flying {
                *by_distance.entry((event.kind, event.distance)).or_default() += 1;
            }
        }
        println!("columns sent more than once: {sent_again}");
        println!(
            "light updates for held columns: {}, changing no row: {}",
            self.light_updates, self.light_updates_changing_nothing
        );
        println!("sequences per column:");
        for (sequence, count) in sequences {
            println!("  {count:6} {sequence}");
        }
        println!("while flying, by kind and distance from the bot:");
        for ((kind, distance), count) in by_distance {
            println!("  {kind:7} {distance:3} {count}");
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let seconds: u32 = args.get(1).map_or(Ok(30), |count| count.parse())?;
    let speed: f64 = args.get(2).map_or(Ok(1.0), |count| count.parse())?;
    let simulation: Option<u8> = args
        .get(3)
        .filter(|distance| *distance != "-")
        .map(|distance| distance.parse())
        .transpose()?;
    let view_distance: u8 = args.get(4).map_or(Ok(12), |count| count.parse())?;

    let mut plugins = MinecraftServerPlugins::default();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.network = ServerNetworkPlugin::embedded();
    plugins.network.announce_on_lan = false;
    plugins.worlds.default_game_mode = GameMode::Creative;
    let mut app = App::new();
    app.add_plugins(plugins.build());
    if let Some(horizontal) = simulation {
        app.add_dimension_plugin(Simulation(SimulationDistance {
            horizontal,
            vertical: u8::MAX,
        }));
    }
    let address = app.world().resource::<BoundAddress>().0;
    let stop = Arc::new(AtomicBool::new(false));
    let server_stop = stop.clone();
    let server = mcrs_minecraft_engine_server::spawn_server_thread(app, move |mut app| {
        app.finish();
        app.cleanup();
        while !server_stop.load(Ordering::Relaxed) {
            let started = Instant::now();
            app.update();
            runner::between_ticks(&mut app);
            runner::drain_until(&mut app, started + TICK);
        }
    })?;
    let runtime = tokio::runtime::Runtime::new()?;
    let ticks = WARMUP_TICKS + seconds * 20;
    let trace = runtime.block_on(fly(address, ticks, speed, view_distance));
    stop.store(true, Ordering::Relaxed);
    server.join().ok();
    trace?.report();
    Ok(())
}

async fn fly(
    address: std::net::SocketAddr,
    ticks: u32,
    speed: f64,
    view_distance: u8,
) -> anyhow::Result<Trace> {
    let io = PacketIo::connect(address).await?;
    let Joined { mut connection, .. } = join(
        io,
        Join {
            server_address: address.ip().to_string(),
            server_port: address.port(),
            remote_addr: address,
            username: "tracer".to_owned(),
            profile_id: Uuid::nil(),
            protocol_version: VERSION.protocol_version,
            view_distance,
            holds_vanilla_pack: false,
        },
    )
    .await?;
    let step = DVec3::new(speed, 0.0, 0.0);
    let mut position: Option<DVec3> = None;
    let mut trace = Trace::default();
    let mut interval = tokio::time::interval(TICK);
    for tick in 0..ticks {
        interval.tick().await;
        let bot = position.map_or(ColumnPos::new(0, 0), |at| {
            ColumnPos::new((at.x / 16.0).floor() as i32, (at.z / 16.0).floor() as i32)
        });
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
                connection.write_packet(&ServerboundChunkBatchReceived {
                    desired_chunks_per_tick: 64.0,
                });
            } else if packet.id == ClientboundKeepAlive::ID {
                let keep_alive = ClientboundKeepAlive::decode(body)?;
                connection.write_packet(&ServerboundKeepAlive(KeepAlive {
                    payload: keep_alive.0.payload,
                }));
            } else {
                trace.record(&packet, tick, bot)?;
            }
        }
        if let Some(at) = position.as_mut()
            && tick > WARMUP_TICKS
        {
            *at += step;
            connection.write_packet(&ServerboundMovePlayerPos {
                position: (*at).into(),
                flags: MoveFlags::new(),
            });
        }
        connection.flush()?;
    }
    Ok(trace)
}
