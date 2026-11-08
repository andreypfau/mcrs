use std::collections::HashSet;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::bail;
use glam::DVec3;
use mcrs_minecraft_core::VERSION;
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
    ClientboundChunkBatchFinished, ClientboundForgetLevelChunk, ClientboundLevelChunkWithLight,
    ClientboundPing, ClientboundPlayerPosition,
};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundChunkBatchReceived, ServerboundClientTickEnd,
    ServerboundMovePlayerPos, ServerboundPlayerAbilities, ServerboundPlayerLoaded, ServerboundPong,
};
use mcrs_voxel::ColumnPos;
use serde::Serialize;
use tokio::time::{Instant, MissedTickBehavior};

const TICK: Duration = Duration::from_millis(50);
pub const VIEW_DISTANCE: u8 = 10;
/// Both servers clamp the asked rate to their own ceiling, so asking for the most leaves the
/// pace to the server's batch window, the same rule on both.
const DESIRED_CHUNKS_PER_TICK: f32 = 64.0;
const ALTITUDE: f64 = 24.0;
const WALK_RADIUS: f64 = 48.0;
const WALK_BLOCKS_PER_SECOND: f64 = 5.6;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Behaviour {
    Walk,
    Explore,
    Idle,
    Mixed,
}

impl Behaviour {
    /// `Mixed` gives 60 % of the bots `Walk`, 20 % `Explore` and 20 % `Idle`.
    pub fn of_bot(self, index: usize) -> Behaviour {
        match (self, index % 5) {
            (Behaviour::Mixed, 3) => Behaviour::Explore,
            (Behaviour::Mixed, 4) => Behaviour::Idle,
            (Behaviour::Mixed, _) => Behaviour::Walk,
            (behaviour, _) => behaviour,
        }
    }
}

impl FromStr for Behaviour {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "walk" => Behaviour::Walk,
            "explore" => Behaviour::Explore,
            "idle" => Behaviour::Idle,
            "mixed" => Behaviour::Mixed,
            other => bail!("unknown behaviour {other}"),
        })
    }
}

pub struct Plan {
    pub address: SocketAddr,
    pub index: usize,
    pub behaviour: Behaviour,
    pub explore_blocks_per_second: f64,
    pub measure_from: Instant,
    pub measure_until: Instant,
}

/// What one bot saw. `held_before` and `received_during` are column sets, so the run can count
/// the columns that no bot received before the measured window.
#[derive(Default)]
pub struct BotReport {
    pub columns_during: u64,
    pub resent_during: u64,
    pub pulled_back_during: u64,
    pub moves_during: u64,
    pub waits_during: u64,
    pub held_before: HashSet<ColumnPos>,
    pub received_during: HashSet<ColumnPos>,
}

pub async fn run(plan: Plan) -> anyhow::Result<BotReport> {
    let mut connection = connect(plan.address, plan.index).await?;
    let behaviour = plan.behaviour.of_bot(plan.index);
    let mut random = Random(plan.index as u64 * 0x9e37_79b9_7f4a_7c15 + 1);
    let heading = plan.index as f64 * 2.399_963;
    let mut report = BotReport::default();
    let mut held: HashSet<ColumnPos> = HashSet::new();
    let mut position: Option<DVec3> = None;
    let mut spawn = DVec3::ZERO;
    let mut target = DVec3::ZERO;
    let mut climbed = false;
    let mut ticks = tokio::time::interval(TICK);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let now = ticks.tick().await;
        if now >= plan.measure_until {
            return Ok(report);
        }
        let measuring = now >= plan.measure_from;
        while let Some(packet) = connection.try_recv()? {
            let body = &mut &packet.payload[..];
            if packet.id == ClientboundLevelChunkWithLight::ID {
                let column = ColumnPos::decode(body)?;
                let fresh = held.insert(column);
                if measuring {
                    report.columns_during += 1;
                    report.resent_during += u64::from(!fresh);
                    report.received_during.insert(column);
                } else {
                    report.held_before.insert(column);
                }
            } else if packet.id == ClientboundForgetLevelChunk::ID {
                let forget = ClientboundForgetLevelChunk::decode(body)?;
                held.remove(&ColumnPos {
                    x: forget.x,
                    z: forget.z,
                });
            } else if packet.id == ClientboundChunkBatchFinished::ID {
                connection.write_packet(&ServerboundChunkBatchReceived {
                    desired_chunks_per_tick: DESIRED_CHUNKS_PER_TICK,
                });
            } else if packet.id == ClientboundKeepAlive::ID {
                let keep_alive = ClientboundKeepAlive::decode(body)?;
                connection.write_packet(&ServerboundKeepAlive(KeepAlive {
                    payload: keep_alive.0.payload,
                }));
            } else if packet.id == ClientboundPing::ID {
                let ping = ClientboundPing::decode(body)?;
                connection.write_packet(&ServerboundPong(Pong { payload: ping.id }));
            } else if packet.id == ClientboundPlayerPosition::ID {
                let teleport = ClientboundPlayerPosition::decode(body)?;
                connection.write_packet(&ServerboundAcceptTeleportation {
                    teleport_id: teleport.teleport_id,
                    position: teleport.position.into(),
                    look: teleport.look,
                });
                if position.is_none() {
                    spawn = teleport.position;
                    target = spawn + DVec3::Y * ALTITUDE;
                    connection.write_packet(&ServerboundPlayerLoaded);
                    connection.write_packet(&ServerboundPlayerAbilities { flying: true });
                } else if measuring {
                    report.pulled_back_during += 1;
                }
                position = Some(teleport.position);
            }
        }
        match position.as_mut() {
            None => {}
            Some(at) if !held.contains(&ColumnPos::from(*at)) => {
                report.waits_during += u64::from(measuring);
            }
            Some(at) => {
                let speed = if climbed && behaviour == Behaviour::Explore {
                    plan.explore_blocks_per_second
                } else {
                    WALK_BLOCKS_PER_SECOND
                };
                if at.distance(target) < 0.01 {
                    climbed = true;
                    target = next_target(behaviour, *at, spawn, heading, &mut random);
                }
                *at = step_toward(*at, target, speed / 20.0);
                connection.write_packet(&ServerboundMovePlayerPos {
                    position: (*at).into(),
                    flags: MoveFlags::new(),
                });
                report.moves_during += u64::from(measuring);
            }
        }
        connection.write_packet(&ServerboundClientTickEnd);
        connection.flush()?;
    }
}

async fn connect(address: SocketAddr, index: usize) -> anyhow::Result<RawConnection> {
    let io = PacketIo::connect(address).await?;
    let Joined { connection, .. } = join(
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
    Ok(connection)
}

/// Where a bot that reached its target goes next, at the height of its climb.
fn next_target(
    behaviour: Behaviour,
    at: DVec3,
    spawn: DVec3,
    heading: f64,
    random: &mut Random,
) -> DVec3 {
    match behaviour {
        Behaviour::Walk => {
            let angle = random.next_unit() * std::f64::consts::TAU;
            let radius = random.next_unit().sqrt() * WALK_RADIUS;
            DVec3::new(
                spawn.x + angle.cos() * radius,
                spawn.y + ALTITUDE,
                spawn.z + angle.sin() * radius,
            )
        }
        Behaviour::Explore => at + DVec3::new(heading.cos(), 0.0, heading.sin()) * 1.0e6,
        Behaviour::Idle | Behaviour::Mixed => at,
    }
}

fn step_toward(from: DVec3, to: DVec3, most: f64) -> DVec3 {
    let gap = to - from;
    let length = gap.length();
    if length <= most {
        to
    } else {
        from + gap * (most / length)
    }
}

/// xorshift64, so each bot walks the same points on both servers.
struct Random(u64);

impl Random {
    fn next_unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}
