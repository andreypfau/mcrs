use std::collections::HashSet;
use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::bail;
use glam::DVec3;
use mcrs_minecraft_bench_support::{TICK, Upkeep, announce_loaded, answer_upkeep, connect};
use mcrs_minecraft_protocol::{Decode, Packet, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundForgetLevelChunk, ClientboundLevelChunkWithLight,
};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundClientTickEnd, ServerboundMovePlayerPos,
};
use mcrs_voxel::ColumnPos;
use serde::Serialize;
use tokio::time::{Instant, MissedTickBehavior};

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
    let mut connection = connect(plan.address, format!("bot{}", plan.index)).await?;
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
            } else if let Upkeep::Teleported(teleported) =
                answer_upkeep(&mut connection, packet.id, body)?
            {
                if position.is_none() {
                    spawn = teleported;
                    target = spawn + DVec3::Y * ALTITUDE;
                    announce_loaded(&mut connection);
                } else if measuring {
                    report.pulled_back_during += 1;
                }
                position = Some(teleported);
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
