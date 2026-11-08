use bytes::BytesMut;
use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundEntityPositionSync, ClientboundMoveEntityPos,
    ClientboundMoveEntityPosRot, ClientboundMoveEntityRot, ClientboundRotateHead,
    ClientboundSetEntityData, ClientboundSetEntityMotion, DeltaStep, PositionPath, PositionStep,
    VecDelta,
};
use mcrs_minecraft_protocol_game::entity::Metadata;
use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3};
use uuid::Uuid;

use crate::delta_base::{all_or_nothing, position_delta};
use crate::mob_bundle::{health_entry, pose_entry};
use crate::mob_state::MobState;

/// The squared move, in blocks, from which the position counts as changed (TRK-UPD-003).
const MOVED: f64 = 7.629_394_5e-6_f32 as f64;
/// A resting subject sends its position each this many tracker ticks.
const POSITION_REFRESH: u64 = 60;
/// The updates after which the next one sends the absolute position.
const UPDATES_PER_SYNC: u32 = 400;
/// The squared change of the velocity, in blocks per tick, from which it is sent (TRK-UPD-007).
const VELOCITY_CHANGE: f64 = 1.0e-7;

/// The tracker of one mob, which all its viewers share (TRK-UPD-001 to TRK-UPD-009): what the
/// viewers last received, and the open step of the path of the next update.
#[derive(Clone, Debug)]
pub(crate) struct MobTracker {
    interval: u64,
    /// The tick of the first tracker tick.
    start: u64,
    /// The tick of the last tracker tick that ran.
    last: u64,
    latest: MobState,
    /// The position base.
    base: DVec3,
    velocity: DVec3,
    yaw: ByteAngle,
    pitch: ByteAngle,
    head_yaw: ByteAngle,
    on_ground: bool,
    health: f32,
    dying: bool,
    /// The updates since the last `entity_position_sync`.
    since_sync: u32,
    /// The tracker ticks of the open step.
    open: u32,
}

impl MobTracker {
    /// The tracker of a mob that `state` shows, whose first tracker tick is `tick`.
    pub(crate) fn new(state: MobState, interval: u32, tick: u64) -> Self {
        let mut tracker = Self {
            interval: u64::from(interval.max(1)),
            start: tick,
            last: tick.wrapping_sub(1),
            latest: state,
            base: state.position,
            velocity: state.velocity,
            yaw: ByteAngle::default(),
            pitch: ByteAngle::default(),
            head_yaw: ByteAngle::default(),
            on_ground: state.on_ground,
            health: state.health,
            dying: state.dying,
            since_sync: 0,
            open: 0,
        };
        tracker.restart();
        tracker
    }

    pub(crate) fn latest(&self) -> &MobState {
        &self.latest
    }

    /// The health and the dying state that the viewers last received.
    pub(crate) fn sent_data(&self) -> (f32, bool) {
        (self.health, self.dying)
    }

    pub(crate) fn record(&mut self, state: MobState) {
        self.latest = state;
    }

    /// Whether the tracker skipped a tracker tick before `tick`: no client held the mob, so
    /// what its viewers last received is no longer known.
    pub(crate) fn is_stale(&self, tick: u64) -> bool {
        tick.wrapping_sub(self.last) > 1
    }

    /// Makes the latest state the state that the viewers last received, for the first viewer of
    /// a mob that no client held.
    pub(crate) fn restart(&mut self) {
        let state = self.latest;
        self.base = state.position;
        self.velocity = state.velocity;
        self.yaw = ByteAngle::from_degrees(state.yaw);
        self.pitch = ByteAngle::from_degrees(state.pitch);
        self.head_yaw = ByteAngle::from_degrees(state.head_yaw);
        self.on_ground = state.on_ground;
        self.health = state.health;
        self.dying = state.dying;
        self.since_sync = 0;
        self.open = 0;
    }

    /// Whether an entity data entry changed after the tracker last sent it (TRK-DATA-005).
    fn dirty(&self) -> bool {
        self.latest.health.to_bits() != self.health.to_bits() || self.latest.dying != self.dying
    }

    /// The `add_entity` of the spawn bundle: the base and the last sent velocity and angles.
    pub(crate) fn add_entity(
        &self,
        id: VarInt,
        uuid: Uuid,
        kind: EntityType,
    ) -> ClientboundAddEntity {
        ClientboundAddEntity {
            id,
            uuid,
            kind,
            pos: self.base,
            movement: LpVec3(self.velocity),
            pitch: self.pitch,
            yaw: self.yaw,
            head_yaw: self.head_yaw,
            data: VarInt(0),
        }
    }

    /// Runs the tracker tick of `tick` and appends the packets of its update, if it is one, to
    /// `bytes`. The tracker and `bytes` stay as they are when an encoding fails.
    pub(crate) fn tick(&mut self, id: VarInt, tick: u64, bytes: &mut BytesMut) -> Result<()> {
        let count = tick.wrapping_sub(self.start);
        if count % self.interval == 0 || self.dirty() {
            let mut next = self.clone();
            all_or_nothing(bytes, |bytes| next.update(id, count, bytes))?;
            *self = next;
        }
        self.open = self.open.saturating_add(1);
        self.last = tick;
        Ok(())
    }

    fn update(&mut self, id: VarInt, count: u64, bytes: &mut BytesMut) -> Result<()> {
        let state = self.latest;
        let change = (state.velocity - self.velocity).length_squared();
        if change > VELOCITY_CHANGE || (change > 0.0 && state.velocity.length_squared() == 0.0) {
            write_record(
                bytes,
                &ClientboundSetEntityMotion {
                    entity_id: id,
                    velocity: LpVec3(state.velocity),
                },
            )?;
            self.velocity = state.velocity;
        }
        let yaw = ByteAngle::from_degrees(state.yaw);
        let pitch = ByteAngle::from_degrees(state.pitch);
        let position_due =
            (state.position - self.base).length_squared() >= MOVED || count % POSITION_REFRESH == 0;
        let rotation_due = (yaw, pitch) != (self.yaw, self.pitch);
        let delta = self.delta(state.position);
        let synced = match delta {
            _ if self.since_sync > UPDATES_PER_SYNC || state.on_ground != self.on_ground => {
                self.sync(id, bytes)?;
                true
            }
            None if position_due => {
                self.sync(id, bytes)?;
                true
            }
            Some(delta) if position_due && rotation_due => {
                write_record(
                    bytes,
                    &ClientboundMoveEntityPosRot {
                        entity_id: id,
                        delta,
                        y_rot: yaw,
                        x_rot: pitch,
                        on_ground: state.on_ground,
                    },
                )?;
                false
            }
            Some(delta) if position_due => {
                write_record(
                    bytes,
                    &ClientboundMoveEntityPos {
                        entity_id: id,
                        delta,
                        on_ground: state.on_ground,
                    },
                )?;
                false
            }
            _ if rotation_due => {
                write_record(
                    bytes,
                    &ClientboundMoveEntityRot {
                        entity_id: id,
                        on_ground: state.on_ground,
                        y_rot: yaw,
                        x_rot: pitch,
                    },
                )?;
                false
            }
            _ => false,
        };
        if position_due {
            self.base = state.position;
        }
        if rotation_due {
            self.yaw = yaw;
            self.pitch = pitch;
        }
        self.since_sync = if synced {
            0
        } else {
            self.since_sync.saturating_add(1)
        };
        if self.dirty() {
            let mut entries = Vec::new();
            if state.dying != self.dying {
                entries.push(pose_entry());
            }
            if state.health.to_bits() != self.health.to_bits() {
                entries.push(health_entry(state.health));
            }
            write_record(
                bytes,
                &ClientboundSetEntityData {
                    entity_id: id,
                    metadata: Metadata(entries),
                },
            )?;
            self.health = state.health;
            self.dying = state.dying;
        }
        let head_yaw = ByteAngle::from_degrees(state.head_yaw);
        if head_yaw != self.head_yaw {
            write_record(
                bytes,
                &ClientboundRotateHead {
                    entity_id: id,
                    y_head_rot: head_yaw,
                },
            )?;
            self.head_yaw = head_yaw;
        }
        self.open = 0;
        Ok(())
    }

    /// The relative path from the base to `position`: one step of the open ticks, or a linear
    /// move when the step is empty (TRK-UPD-005). `None` when it does not fit (TRK-WIRE-001).
    fn delta(&self, position: DVec3) -> Option<VecDelta> {
        let delta = position_delta(self.base, position)?;
        Some(match self.open {
            0 => VecDelta::Linear(delta),
            ticks => VecDelta::Stepped {
                steps: vec![DeltaStep {
                    ticks: VarInt(i32::try_from(ticks).unwrap_or(i32::MAX)),
                    delta,
                }],
                start_immediately: false,
            },
        })
    }

    /// Writes the absolute position and the rotation, and moves every sent value there.
    fn sync(&mut self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        let state = self.latest;
        let position = match self.open {
            0 => PositionPath::Linear(state.position),
            ticks => PositionPath::Stepped {
                steps: vec![PositionStep {
                    position: state.position,
                    tick_offset: VarInt(i32::try_from(ticks).unwrap_or(i32::MAX)),
                }],
                start_immediately: false,
            },
        };
        write_record(
            bytes,
            &ClientboundEntityPositionSync {
                entity_id: id,
                position,
                look: Look {
                    yaw: state.yaw,
                    pitch: state.pitch,
                },
                on_ground: state.on_ground,
            },
        )?;
        self.base = state.position;
        self.yaw = ByteAngle::from_degrees(state.yaw);
        self.pitch = ByteAngle::from_degrees(state.pitch);
        self.on_ground = state.on_ground;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::encode::record;

    const ID: VarInt = VarInt(51);
    const FLOOR: f64 = -60.0;
    const RESTING: f64 = -0.0784;

    /// The states of a mob that falls from `y = -40` onto a floor at `y = -60`, one for each
    /// tick: it moves by its velocity, then gravity and drag change the velocity.
    fn fall(ticks: usize) -> Vec<MobState> {
        let mut y = -40.0;
        let mut velocity = 0.0;
        let mut states = Vec::new();
        for _ in 0..ticks {
            y += velocity;
            let on_ground = y <= FLOOR;
            if on_ground {
                y = FLOOR;
                velocity = 0.0;
            }
            velocity = (velocity - 0.08) * 0.98;
            states.push(MobState {
                position: DVec3::new(0.5, y, 4.5),
                velocity: DVec3::new(0.0, velocity, 0.0),
                on_ground,
                ..MobState::default()
            });
        }
        states
    }

    fn motion(velocity: f64) -> Bytes {
        record(&ClientboundSetEntityMotion {
            entity_id: ID,
            velocity: LpVec3(DVec3::new(0.0, velocity, 0.0)),
        })
        .unwrap()
    }

    fn moved(delta: VecDelta, on_ground: bool) -> Bytes {
        record(&ClientboundMoveEntityPos {
            entity_id: ID,
            delta,
            on_ground,
        })
        .unwrap()
    }

    fn step(ticks: i32, dy: i16) -> VecDelta {
        VecDelta::Stepped {
            steps: vec![DeltaStep {
                ticks: VarInt(ticks),
                delta: [0, dy, 0],
            }],
            start_immediately: false,
        }
    }

    /// TRK-E2: the updates of a falling zombified piglin, from its first tracker tick on.
    #[test]
    fn trk_upd_a_falling_mob_updates_each_three_ticks_and_syncs_at_the_landing() {
        let states = fall(70);
        let mut tracker = MobTracker::new(states[0], 3, 0);
        let mut updates: Vec<(u64, Bytes)> = Vec::new();
        for (tick, state) in (0..).zip(&states) {
            tracker.record(*state);
            let mut out = BytesMut::new();
            tracker.tick(ID, tick, &mut out).unwrap();
            if !out.is_empty() {
                updates.push((tick, out.freeze()));
            }
        }

        assert_eq!(
            updates[0],
            (0, moved(VecDelta::Linear([0, 0, 0]), false)),
            "the tracker starts at the first state: no motion, a linear zero move"
        );
        assert_eq!(
            updates[1],
            (
                3,
                [motion(states[3].velocity.y), moved(step(3, -1901), false)]
                    .concat()
                    .into()
            ),
            "the velocity, then one step of three ticks"
        );
        assert_eq!(
            updates[2],
            (
                6,
                [motion(states[6].velocity.y), moved(step(3, -4622), false)]
                    .concat()
                    .into()
            )
        );
        let landing = states.iter().position(|state| state.on_ground).unwrap() as u64;
        let (tick, sync) = updates
            .iter()
            .find(|(tick, _)| *tick >= landing)
            .unwrap()
            .clone();
        let expected_sync = record(&ClientboundEntityPositionSync {
            entity_id: ID,
            position: PositionPath::Stepped {
                steps: vec![PositionStep {
                    position: DVec3::new(0.5, FLOOR, 4.5),
                    tick_offset: VarInt(3),
                }],
                start_immediately: false,
            },
            look: Look::default(),
            on_ground: true,
        })
        .unwrap();
        assert_eq!(
            sync,
            Bytes::from([motion(RESTING), expected_sync].concat()),
            "the landing at tick {tick} sends the resting velocity and the absolute position"
        );
        assert!(
            updates.iter().all(|(tick, _)| tick % 3 == 0),
            "no update between the intervals"
        );
        assert_eq!(
            updates.last(),
            Some(&(60, moved(step(3, 0), true))),
            "a mob at rest sends a zero step each 60 tracker ticks"
        );
    }
}
