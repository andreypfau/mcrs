//! The watcher: one protocol client that hovers near the pigs, tracks the pigs the server shows
//! it, and times the server's ticks from the packets it gets.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use glam::DVec3;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_network::RawConnection;
use mcrs_minecraft_network::join::{Join, Joined, join};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_protocol::packets::common::serverbound::{KeepAlive, Pong};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundKeepAlive;
use mcrs_minecraft_protocol::packets::game::serverbound::ServerboundKeepAlive;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Packet, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundChunkBatchFinished, ClientboundEntityPositionSync,
    ClientboundMoveEntityPos, ClientboundMoveEntityPosRot, ClientboundPing,
    ClientboundPlayerPosition, ClientboundRemoveEntities, ClientboundSetTime,
    ClientboundTeleportEntity, PositionPath, VecDelta,
};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundChunkBatchReceived, ServerboundClientTickEnd,
    ServerboundMovePlayerPos, ServerboundPlayerAbilities, ServerboundPlayerLoaded, ServerboundPong,
};
use tokio::sync::{mpsc, oneshot};
use tokio::time::MissedTickBehavior;

use crate::scene::WATCHER_NAME;

const TICK: Duration = Duration::from_millis(50);
const POLL: Duration = Duration::from_millis(5);
const VIEW_DISTANCE: u8 = 10;
const DESIRED_CHUNKS_PER_TICK: f32 = 64.0;
const DELTA_STEPS_PER_BLOCK: f64 = 4096.0;

#[derive(Clone, Copy, Debug)]
pub struct PigTrack {
    pub position: DVec3,
    /// The length of the path the packets moved the pig along since it was added.
    pub path: f64,
}

pub struct PigSnapshot {
    pub at: std::time::Instant,
    pub pigs: HashMap<i32, PigTrack>,
    pub move_packets: u64,
    /// The server placed the watcher's player, so it is in the world.
    pub placed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TimeSample {
    pub at: std::time::Instant,
    pub game_time: i64,
}

#[derive(Default)]
pub struct WatcherLog {
    pub times: Vec<TimeSample>,
}

enum Request {
    Snapshot(oneshot::Sender<PigSnapshot>),
    Stop,
}

pub struct Watcher {
    requests: mpsc::UnboundedSender<Request>,
    task: tokio::task::JoinHandle<anyhow::Result<WatcherLog>>,
}

impl Watcher {
    pub fn spawn(runtime: &tokio::runtime::Runtime, address: SocketAddr) -> Self {
        let (requests, receiver) = mpsc::unbounded_channel();
        let task = runtime.spawn(run(address, receiver));
        Self { requests, task }
    }

    /// The pigs the watcher tracks now, or `None` once it stopped.
    pub fn snapshot(&self, runtime: &tokio::runtime::Runtime) -> Option<PigSnapshot> {
        let (reply, answer) = oneshot::channel();
        self.requests.send(Request::Snapshot(reply)).ok()?;
        runtime.block_on(answer).ok()
    }

    pub fn finished(&self) -> bool {
        self.task.is_finished()
    }

    pub fn stop(self, runtime: &tokio::runtime::Runtime) -> anyhow::Result<WatcherLog> {
        let _ = self.requests.send(Request::Stop);
        runtime.block_on(self.task)?
    }
}

struct State {
    pigs: HashMap<i32, PigTrack>,
    move_packets: u64,
    log: WatcherLog,
    position: Option<DVec3>,
}

async fn run(
    address: SocketAddr,
    mut requests: mpsc::UnboundedReceiver<Request>,
) -> anyhow::Result<WatcherLog> {
    let mut connection = connect(address)
        .await
        .context("the watcher could not join")?;
    let mut state = State {
        pigs: HashMap::new(),
        move_packets: 0,
        log: WatcherLog::default(),
        position: None,
    };
    let mut ticks = tokio::time::interval(TICK);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut poll = tokio::time::interval(POLL);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = poll.tick() => receive(&mut connection, &mut state)?,
            _ = ticks.tick() => {
                if let Some(at) = state.position {
                    connection.write_packet(&ServerboundMovePlayerPos {
                        position: at.into(),
                        flags: MoveFlags::new(),
                    });
                }
                connection.write_packet(&ServerboundClientTickEnd);
                connection.flush()?;
            }
            request = requests.recv() => match request {
                Some(Request::Snapshot(reply)) => {
                    let _ = reply.send(PigSnapshot {
                        at: std::time::Instant::now(),
                        pigs: state.pigs.clone(),
                        move_packets: state.move_packets,
                        placed: state.position.is_some(),
                    });
                }
                Some(Request::Stop) | None => return Ok(state.log),
            },
        }
    }
}

fn receive(connection: &mut RawConnection, state: &mut State) -> anyhow::Result<()> {
    while let Some(packet) = connection.try_recv()? {
        let body = &mut &packet.payload[..];
        let id = packet.id;
        if id == ClientboundAddEntity::ID {
            let add = ClientboundAddEntity::decode(body)?;
            if add.kind == EntityType::Pig {
                state.pigs.insert(
                    add.id.0,
                    PigTrack {
                        position: add.pos,
                        path: 0.0,
                    },
                );
            }
        } else if id == ClientboundRemoveEntities::ID {
            for entity in ClientboundRemoveEntities::decode(body)?.entity_ids {
                state.pigs.remove(&entity.0);
            }
        } else if id == ClientboundMoveEntityPos::ID {
            let moved = ClientboundMoveEntityPos::decode(body)?;
            state.moved_by(moved.entity_id.0, &moved.delta);
        } else if id == ClientboundMoveEntityPosRot::ID {
            let moved = ClientboundMoveEntityPosRot::decode(body)?;
            state.moved_by(moved.entity_id.0, &moved.delta);
        } else if id == ClientboundEntityPositionSync::ID {
            let sync = ClientboundEntityPositionSync::decode(body)?;
            let end = match &sync.position {
                PositionPath::Linear(position) => Some(*position),
                PositionPath::Stepped { steps, .. } => steps.last().map(|step| step.position),
            };
            if let Some(end) = end {
                state.moved_to(sync.entity_id.0, end);
            }
        } else if id == ClientboundTeleportEntity::ID {
            let teleport = ClientboundTeleportEntity::decode(body)?;
            state.moved_to(teleport.entity_id.0, teleport.position);
        } else if id == ClientboundSetTime::ID {
            let time = ClientboundSetTime::decode(body)?;
            state.log.times.push(TimeSample {
                at: packet.timestamp,
                game_time: time.game_time,
            });
        } else if id == ClientboundChunkBatchFinished::ID {
            connection.write_packet(&ServerboundChunkBatchReceived {
                desired_chunks_per_tick: DESIRED_CHUNKS_PER_TICK,
            });
        } else if id == ClientboundKeepAlive::ID {
            let keep_alive = ClientboundKeepAlive::decode(body)?;
            connection.write_packet(&ServerboundKeepAlive(KeepAlive {
                payload: keep_alive.0.payload,
            }));
        } else if id == ClientboundPing::ID {
            let ping = ClientboundPing::decode(body)?;
            connection.write_packet(&ServerboundPong(Pong { payload: ping.id }));
        } else if id == ClientboundPlayerPosition::ID {
            let teleport = ClientboundPlayerPosition::decode(body)?;
            connection.write_packet(&ServerboundAcceptTeleportation {
                teleport_id: teleport.teleport_id,
                position: teleport.position.into(),
                look: teleport.look,
            });
            if state.position.is_none() {
                connection.write_packet(&ServerboundPlayerLoaded);
                connection.write_packet(&ServerboundPlayerAbilities { flying: true });
            }
            state.position = Some(teleport.position);
        }
    }
    connection.flush()
}

impl State {
    fn moved_by(&mut self, entity: i32, delta: &VecDelta) {
        let Some(track) = self.pigs.get_mut(&entity) else {
            return;
        };
        self.move_packets += 1;
        let deltas: Vec<[i16; 3]> = match delta {
            VecDelta::Linear(delta) => vec![*delta],
            VecDelta::Stepped { steps, .. } => steps.iter().map(|step| step.delta).collect(),
        };
        for delta in deltas {
            let step = DVec3::new(
                f64::from(delta[0]),
                f64::from(delta[1]),
                f64::from(delta[2]),
            ) / DELTA_STEPS_PER_BLOCK;
            track.position += step;
            track.path += step.length();
        }
    }

    fn moved_to(&mut self, entity: i32, position: DVec3) {
        let Some(track) = self.pigs.get_mut(&entity) else {
            return;
        };
        self.move_packets += 1;
        track.path += position.distance(track.position);
        track.position = position;
    }
}

async fn connect(address: SocketAddr) -> anyhow::Result<RawConnection> {
    let io = PacketIo::connect(address).await?;
    let Joined { connection, .. } = join(
        io,
        Join {
            server_address: address.ip().to_string(),
            server_port: address.port(),
            remote_addr: address,
            username: WATCHER_NAME.to_owned(),
            profile_id: Uuid::nil(),
            protocol_version: VERSION.protocol_version,
            view_distance: VIEW_DISTANCE,
            holds_vanilla_pack: false,
        },
    )
    .await?;
    Ok(connection)
}
