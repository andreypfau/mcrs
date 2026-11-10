//! The watcher: one protocol client that hovers near the pigs, tracks the pigs the server shows
//! it, and times the server's ticks from the packets it gets.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::Context;
use glam::DVec3;
use mcrs_minecraft_bench_support::{
    TICK, Upkeep, announce_loaded, answer_upkeep, connect, unix_now,
};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_network::RawConnection;
use mcrs_minecraft_protocol::{Decode, Packet, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundEntityPositionSync, ClientboundGameEvent, ClientboundLogin,
    ClientboundMoveEntityPos, ClientboundMoveEntityPosRot, ClientboundRemoveEntities,
    ClientboundSetTime, ClientboundTeleportEntity, PositionPath, VecDelta,
};
use mcrs_minecraft_protocol_game::game_event::GameEventKind;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundClientTickEnd, ServerboundMovePlayerPos,
};
use tokio::sync::{mpsc, oneshot};
use tokio::time::MissedTickBehavior;

use crate::scene::WATCHER_NAME;

const POLL: Duration = Duration::from_millis(1);
/// The reader hands packets over through a channel of 256, so a poll drains it, yields to the
/// reader and drains again: 50,000 pigs send more packets than one drain a millisecond takes.
const DRAINS_PER_POLL: usize = 64;
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
    /// The game mode of the last login or game mode change packet.
    pub game_mode: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct TimeSample {
    pub at: std::time::Instant,
    pub game_time: i64,
}

#[derive(Default)]
pub struct WatcherLog {
    pub times: Vec<TimeSample>,
    /// Why the watcher stopped before it was asked to.
    pub error: Option<String>,
}

enum Request {
    Snapshot(oneshot::Sender<PigSnapshot>),
    Placed(oneshot::Sender<bool>),
    Stop,
}

pub struct Watcher {
    requests: mpsc::UnboundedSender<Request>,
    task: tokio::task::JoinHandle<WatcherLog>,
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

    /// Whether the server placed the watcher's player; `false` once it stopped.
    pub fn placed(&self, runtime: &tokio::runtime::Runtime) -> bool {
        let (reply, answer) = oneshot::channel();
        self.requests.send(Request::Placed(reply)).is_ok()
            && runtime.block_on(answer).unwrap_or(false)
    }

    pub fn finished(&self) -> bool {
        self.task.is_finished()
    }

    pub fn stop(self, runtime: &tokio::runtime::Runtime) -> WatcherLog {
        let _ = self.requests.send(Request::Stop);
        runtime
            .block_on(self.task)
            .unwrap_or_else(|error| WatcherLog {
                times: Vec::new(),
                error: Some(error.to_string()),
            })
    }
}

struct State {
    pigs: HashMap<i32, PigTrack>,
    move_packets: u64,
    log: WatcherLog,
    position: Option<DVec3>,
    game_mode: Option<String>,
}

async fn run(address: SocketAddr, mut requests: mpsc::UnboundedReceiver<Request>) -> WatcherLog {
    let mut state = State {
        pigs: HashMap::new(),
        move_packets: 0,
        log: WatcherLog::default(),
        position: None,
        game_mode: None,
    };
    let result = match connect(address, WATCHER_NAME.to_owned())
        .await
        .context("the watcher could not join")
    {
        Ok(mut connection) => serve(&mut connection, &mut state, &mut requests).await,
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        state.log.error = Some(format!("{error:#} (at unix time {})", unix_now()));
    }
    state.log
}

async fn serve(
    connection: &mut RawConnection,
    state: &mut State,
    requests: &mut mpsc::UnboundedReceiver<Request>,
) -> anyhow::Result<()> {
    let mut ticks = tokio::time::interval(TICK);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut poll = tokio::time::interval(POLL);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = poll.tick() => {
                for _ in 0..DRAINS_PER_POLL {
                    if receive(connection, state)? == 0 {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            }
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
                        game_mode: state.game_mode.clone(),
                    });
                }
                Some(Request::Placed(reply)) => {
                    let _ = reply.send(state.position.is_some());
                }
                Some(Request::Stop) | None => return Ok(()),
            },
        }
    }
}

/// Handles every packet the reader has handed over and returns how many there were.
fn receive(connection: &mut RawConnection, state: &mut State) -> anyhow::Result<usize> {
    let mut received = 0;
    while let Some(packet) = connection.try_recv()? {
        received += 1;
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
        } else if id == ClientboundLogin::ID {
            let login = ClientboundLogin::decode(body)?;
            state.game_mode = Some(format!("{:?}", login.player_spawn_info.game_mode));
        } else if id == ClientboundGameEvent::ID {
            if let GameEventKind::ChangeGameMode(mode) =
                ClientboundGameEvent::decode(body)?.game_event
            {
                state.game_mode = Some(format!("{mode:?}"));
            }
        } else if let Upkeep::Teleported(position) = answer_upkeep(connection, id, body)? {
            if state.position.is_none() {
                announce_loaded(connection);
            }
            state.position = Some(position);
        }
    }
    connection.flush()?;
    Ok(received)
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
