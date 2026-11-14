use bytes::BytesMut;
use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::{ClientMotion, EntityTypeData, PreciseSync};
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundEntityPositionSync, ClientboundRotateHead,
    ClientboundSetEntityData, ClientboundSetEntityMotion, DeltaStep, PositionPath, PositionStep,
    VecDelta,
};
use mcrs_minecraft_protocol_game::entity::Metadata;
use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3, position_delta};
use uuid::Uuid;

use crate::delta_base::all_or_nothing;
use crate::mob_bundle::{Entries, LivingEntries};
use crate::mob_state::MobState;
use crate::move_choice::{Due, MOVED, MoveChoice, POSITION_REFRESH, Resync, UPDATES_PER_SYNC};
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;

/// The squared change of the velocity, in blocks per tick, from which it is sent (TRK-UPD-007).
const VELOCITY_CHANGE: f64 = 1.0e-7;

/// The tracker of one mob, which all its viewers share (TRK-UPD-001 to TRK-UPD-009): what the
/// viewers last received, and the open step of the path of the next update.
#[derive(Clone, Debug)]
pub(crate) struct MobTracker {
    kind: &'static EntityTypeData,
    /// The tick of the first tracker tick.
    start: u64,
    /// The tick of the last tracker tick that ran.
    last: u64,
    latest: MobState,
    /// The latest entity data entries.
    entries: Entries,
    sent: Sent,
    /// The updates since the last `entity_position_sync`.
    since_sync: u32,
    /// The tracker ticks of the open step.
    open: u32,
}

/// The state of a mob that its viewers last received.
#[derive(Clone, Debug)]
struct Sent {
    /// The position base.
    position: DVec3,
    velocity: DVec3,
    yaw: ByteAngle,
    pitch: ByteAngle,
    head_yaw: ByteAngle,
    on_ground: bool,
    entries: Entries,
}

impl Sent {
    fn of(state: &MobState, entries: &Entries) -> Self {
        Self {
            position: state.position,
            velocity: state.velocity,
            yaw: ByteAngle::from_degrees(state.yaw),
            pitch: ByteAngle::from_degrees(state.pitch),
            head_yaw: ByteAngle::from_degrees(state.head_yaw),
            on_ground: state.on_ground,
            entries: entries.clone(),
        }
    }
}

impl MobTracker {
    /// The tracker of an entity that `state` and `entries` show, whose first tracker tick is
    /// `tick`.
    pub(crate) fn new(
        state: MobState,
        entries: Entries,
        kind: &'static EntityTypeData,
        tick: u64,
    ) -> Self {
        Self {
            kind,
            start: tick,
            last: tick.wrapping_sub(1),
            latest: state,
            sent: Sent::of(&state, &entries),
            entries,
            since_sync: 0,
            open: 0,
        }
    }

    pub(crate) fn latest(&self) -> &MobState {
        &self.latest
    }

    /// The entity data entries that the viewers last received.
    pub(crate) fn sent_entries(&self) -> &Entries {
        &self.sent.entries
    }

    /// Records `state`; the living entries follow its health and its dying state.
    pub(crate) fn record(&mut self, state: MobState) {
        self.latest = state;
        if let Entries::Living(living) = &mut self.entries {
            *living = LivingEntries::of(&state);
        }
    }

    /// Records the stack of an item entity.
    pub(crate) fn record_stack(&mut self, stack: ProtoStack<Wire>) {
        if let Entries::Item(item) = &mut self.entries {
            *item = stack;
        }
    }

    /// Whether the tracker skipped a tracker tick before `tick`: no client held the mob, so
    /// what its viewers last received is no longer known.
    pub(crate) fn is_stale(&self, tick: u64) -> bool {
        tick.wrapping_sub(self.last) > 1
    }

    /// Makes the latest state the state that the viewers last received, for the first viewer of
    /// a mob that no client held.
    pub(crate) fn restart(&mut self) {
        self.sent = Sent::of(&self.latest, &self.entries);
        self.since_sync = 0;
        self.open = 0;
    }

    /// Whether an entity data entry changed after the tracker last sent it (TRK-DATA-005).
    fn dirty(&self) -> bool {
        self.entries != self.sent.entries
    }

    /// The `add_entity` of the spawn bundle: the base and the last sent velocity and angles, with
    /// the field `data` of the type.
    pub(crate) fn add_entity(&self, id: VarInt, uuid: Uuid, data: VarInt) -> ClientboundAddEntity {
        ClientboundAddEntity {
            id,
            uuid,
            kind: self.kind.entity_type,
            pos: self.sent.position,
            movement: LpVec3(self.sent.velocity),
            pitch: self.sent.pitch,
            yaw: self.sent.yaw,
            head_yaw: self.sent.head_yaw,
            data,
        }
    }

    /// Runs the tracker tick of `tick` and appends the packets of its update, if it is one, to
    /// `bytes`. The tracker and `bytes` stay as they are when an encoding fails.
    pub(crate) fn tick(&mut self, id: VarInt, tick: u64, bytes: &mut BytesMut) -> Result<()> {
        let interval = u64::from(self.kind.update_interval.max(1));
        if self.latest.bounced {
            self.latest.bounced = false;
            if !self.kind.is_living() {
                let count = tick.wrapping_sub(self.start);
                let jumped = (count / interval).wrapping_add(1).wrapping_mul(interval);
                self.start = tick.wrapping_sub(jumped);
            }
        }
        let count = tick.wrapping_sub(self.start);
        if count.is_multiple_of(interval) || self.dirty() {
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
        let change = (state.velocity - self.sent.velocity).length_squared();
        let velocity_changed =
            change > VELOCITY_CHANGE || (change > 0.0 && state.velocity.length_squared() == 0.0);
        if self.kind.tracks_velocity && velocity_changed {
            write_record(
                bytes,
                &ClientboundSetEntityMotion {
                    entity_id: id,
                    velocity: LpVec3(state.velocity),
                },
            )?;
            self.sent.velocity = state.velocity;
        }
        let yaw = ByteAngle::from_degrees(state.yaw);
        let pitch = ByteAngle::from_degrees(state.pitch);
        let rotated = (yaw, pitch) != (self.sent.yaw, self.sent.pitch);
        let at_rest = matches!(self.kind.client_motion, ClientMotion::Simulated(_))
            && state.position == self.sent.position
            && state.on_ground == self.sent.on_ground
            && !rotated;
        if !at_rest {
            self.write_move(id, count, yaw, pitch, bytes)?;
        }
        if self.entries != self.sent.entries {
            write_record(
                bytes,
                &ClientboundSetEntityData {
                    entity_id: id,
                    metadata: Metadata(self.entries.changed_from(&self.sent.entries)),
                },
            )?;
            self.sent.entries = self.entries.clone();
        }
        let head_yaw = ByteAngle::from_degrees(state.head_yaw);
        if head_yaw != self.sent.head_yaw {
            write_record(
                bytes,
                &ClientboundRotateHead {
                    entity_id: id,
                    y_head_rot: head_yaw,
                },
            )?;
            self.sent.head_yaw = head_yaw;
        }
        self.open = 0;
        Ok(())
    }

    /// Appends the move packet of an update that TRK-UPD-003 selects, and moves the sent values.
    fn write_move(
        &mut self,
        id: VarInt,
        count: u64,
        yaw: ByteAngle,
        pitch: ByteAngle,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let state = self.latest;
        self.since_sync = self.since_sync.saturating_add(1);
        let precise = self.kind.client_motion
            == ClientMotion::Simulated(PreciseSync::AfterCollision)
            && (state.horizontal_collision || state.vertical_collision);
        let resync = if precise
            || self.since_sync > UPDATES_PER_SYNC
            || state.on_ground != self.sent.on_ground
        {
            Resync::Due
        } else {
            Resync::NotDue
        };
        let due = Due {
            position: (state.position - self.sent.position).length_squared() >= MOVED
                || count.is_multiple_of(POSITION_REFRESH),
            rotation: (yaw, pitch) != (self.sent.yaw, self.sent.pitch),
        };
        match MoveChoice::choose(resync, due, || self.delta(state.position)) {
            MoveChoice::Sync => self.sync(id, resync, bytes)?,
            MoveChoice::Relative(relative) => {
                relative.write(id, yaw, pitch, state.on_ground, bytes)?;
            }
            MoveChoice::Still => {}
        }
        if due.position {
            self.sent.position = state.position;
        }
        if due.rotation {
            self.sent.yaw = yaw;
            self.sent.pitch = pitch;
        }
        Ok(())
    }

    /// The tracker ticks of the open step; an entity that is not living has the linear path
    /// (TRK-UPD-005).
    fn open_ticks(&self) -> u32 {
        if self.kind.is_living() { self.open } else { 0 }
    }

    /// Appends the state that the viewers last received, as absolute values: the position base,
    /// the velocity, the head yaw and the entity data. A client that missed updates holds the
    /// same state as every other viewer after it, and the next relative move fits its base.
    pub(crate) fn resend(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        let sent = &self.sent;
        all_or_nothing(bytes, |bytes| {
            write_record(
                bytes,
                &ClientboundEntityPositionSync {
                    entity_id: id,
                    position: PositionPath::Linear(sent.position),
                    look: Look {
                        yaw: sent.yaw.to_degrees(),
                        pitch: sent.pitch.to_degrees(),
                    },
                    on_ground: sent.on_ground,
                },
            )?;
            write_record(
                bytes,
                &ClientboundSetEntityMotion {
                    entity_id: id,
                    velocity: LpVec3(sent.velocity),
                },
            )?;
            if self.kind.is_living() {
                write_record(
                    bytes,
                    &ClientboundRotateHead {
                        entity_id: id,
                        y_head_rot: sent.head_yaw,
                    },
                )?;
            }
            write_record(
                bytes,
                &ClientboundSetEntityData {
                    entity_id: id,
                    metadata: Metadata(sent.entries.all()),
                },
            )
        })
    }

    /// The relative path from the base to `position`: one step of the open ticks, or a linear
    /// move when the step is empty (TRK-UPD-005). `None` when it does not fit (TRK-WIRE-001), or
    /// when the type syncs a move that loses precision and this one does (TRK-UPD-013).
    fn delta(&self, position: DVec3) -> Option<VecDelta> {
        let delta = position_delta::encode(self.sent.position, position)?;
        if self.kind.client_motion == ClientMotion::Simulated(PreciseSync::LostPrecision)
            && slides_imprecisely(&self.latest, delta)
        {
            return None;
        }
        Some(match self.open_ticks() {
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

    /// Writes the absolute position and the rotation, and moves the sent position and rotation
    /// there. Only a resync also moves the sent on-ground flag and ends the update count
    /// (TRK-UPD-003).
    fn sync(&mut self, id: VarInt, resync: Resync, bytes: &mut BytesMut) -> Result<()> {
        let state = self.latest;
        let position = match self.open_ticks() {
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
        self.sent.position = state.position;
        self.sent.yaw = ByteAngle::from_degrees(state.yaw);
        self.sent.pitch = ByteAngle::from_degrees(state.pitch);
        if resync == Resync::Due {
            self.sent.on_ground = state.on_ground;
            self.since_sync = 0;
        }
        Ok(())
    }
}

/// The item condition of TRK-UPD-013: the move `delta` along the face of a collision of the last
/// move ends on a coordinate that the wire does not hold. The axes of a horizontal collision
/// cross.
fn slides_imprecisely(state: &MobState, delta: [i16; 3]) -> bool {
    let [dx, _, dz] = delta;
    let x = loses_precision(state.position.x);
    let z = loses_precision(state.position.z);
    (state.vertical_collision && ((dx != 0 && x) || (dz != 0 && z)))
        || (state.horizontal_collision && ((dx != 0 && z) || (dz != 0 && x)))
}

fn loses_precision(coordinate: f64) -> bool {
    let steps = (coordinate * position_delta::STEPS_PER_BLOCK + 0.5).floor();
    steps / position_delta::STEPS_PER_BLOCK != coordinate
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::encode::record;
    use mcrs_minecraft_protocol_game::clientbound::ClientboundMoveEntityPos;
    use mcrs_minecraft_protocol_game::entity::{MetaDataValue, MetadataEntry};
    use mcrs_minecraft_wire::Decode;
    use mcrs_voxel::BlockPos;

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
        let mut tracker = MobTracker::new(
            states[0],
            Entries::Living(LivingEntries::of(&states[0])),
            &EntityTypeData::ZOMBIFIED_PIGLIN,
            0,
        );
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

    fn sync(position: DVec3, on_ground: bool) -> Bytes {
        record(&ClientboundEntityPositionSync {
            entity_id: ID,
            position: PositionPath::Linear(position),
            look: Look::default(),
            on_ground,
        })
        .unwrap()
    }

    fn linear(from: DVec3, to: DVec3, on_ground: bool) -> Bytes {
        let delta = position_delta::encode(from, to).unwrap();
        moved(VecDelta::Linear(delta), on_ground)
    }

    /// The tracker of a subject of `kind` that starts as `spawn`, with entries that do not change.
    fn simulated(kind: &'static EntityTypeData, spawn: MobState) -> MobTracker {
        MobTracker::new(
            spawn,
            Entries::FallingBlock(BlockPos::new(0, 0, 0)),
            kind,
            0,
        )
    }

    /// The ticks of each update and their bytes, for one state per tick from tick 0.
    fn updates(tracker: &mut MobTracker, states: &[MobState]) -> Vec<(u64, Bytes)> {
        let mut updates = Vec::new();
        for (tick, state) in (0..).zip(states) {
            tracker.record(*state);
            let mut out = BytesMut::new();
            tracker.tick(ID, tick, &mut out).unwrap();
            if !out.is_empty() {
                updates.push((tick, out.freeze()));
            }
        }
        updates
    }

    #[test]
    fn trk_upd_012_013_a_collision_selects_the_sync_of_a_simulated_subject() {
        const LOSES: f64 = 16.158_561_036_770_337;
        let at = |x: f64, z: f64| DVec3::new(x, -60.0, z);
        let item = &EntityTypeData::ITEM;
        let falling_block = &EntityTypeData::FALLING_BLOCK;
        #[derive(Clone, Copy, PartialEq, Debug)]
        enum Form {
            Sync,
            Relative,
        }
        let rows = [
            (
                item,
                at(16.0, 4.5),
                at(LOSES, 4.5),
                [false, true],
                Form::Sync,
                "TRK-UPD-013 vector",
            ),
            (
                item,
                at(4.0, 4.5),
                at(4.5, 4.5),
                [false, true],
                Form::Relative,
                "x = 4.5 is exact",
            ),
            (
                item,
                at(16.0, 4.5),
                at(LOSES, 4.5),
                [false, false],
                Form::Relative,
                "no collision",
            ),
            (
                item,
                at(16.0, 4.5),
                at(LOSES, 4.5),
                [true, false],
                Form::Relative,
                "a wall crosses the axes",
            ),
            (
                item,
                at(LOSES, 4.0),
                at(LOSES, 4.5),
                [true, false],
                Form::Sync,
                "a slide along a wall",
            ),
            (
                falling_block,
                at(4.5, 4.5),
                at(4.5, 4.5) - DVec3::Y * 0.5,
                [false, false],
                Form::Relative,
                "a move without a collision",
            ),
            (
                falling_block,
                at(4.5, 4.5),
                at(4.5, 4.5) - DVec3::Y * 0.5,
                [false, true],
                Form::Sync,
                "the precise mark",
            ),
            (
                falling_block,
                at(4.5, 4.5),
                at(4.7, 4.5),
                [true, false],
                Form::Sync,
                "a horizontal collision",
            ),
        ];
        for (kind, base, position, [horizontal, vertical], form, case) in rows {
            let spawn = MobState {
                position: base,
                on_ground: true,
                ..MobState::default()
            };
            let mut tracker = simulated(kind, spawn);
            let state = MobState {
                position,
                on_ground: true,
                horizontal_collision: horizontal,
                vertical_collision: vertical,
                ..MobState::default()
            };
            let updates = updates(&mut tracker, &[state]);
            let expected = match form {
                Form::Sync => sync(position, true),
                Form::Relative => linear(base, position, true),
            };
            assert_eq!(updates, [(0, expected)], "{case}");
        }
    }

    #[test]
    fn trk_upd_011_016_the_cadence_of_a_simulated_subject() {
        let air = |tick: u32| MobState {
            position: DVec3::new(0.5, -40.0 + 0.01 * f64::from(tick), 4.5),
            ..MobState::default()
        };
        let ground = MobState {
            position: DVec3::new(0.5, -60.0, 4.5),
            on_ground: true,
            vertical_collision: true,
            ..MobState::default()
        };
        let moving: Vec<MobState> = (1..71).map(air).collect();
        let mut bounced = moving.clone();
        bounced[23].bounced = true;
        let rows: [(
            &'static EntityTypeData,
            MobState,
            Vec<MobState>,
            &[u64],
            &str,
        ); 5] = [
            (
                &EntityTypeData::ITEM,
                air(0),
                moving.clone(),
                &[0, 20, 40, 60],
                "a moving item",
            ),
            (
                &EntityTypeData::ITEM,
                air(0),
                bounced,
                &[0, 20, 23, 43, 63],
                "a bounce at c = 23",
            ),
            (
                &EntityTypeData::ITEM,
                air(0),
                vec![ground; 450],
                &[0],
                "an item that lands, then rests",
            ),
            (
                &EntityTypeData::ITEM,
                ground,
                vec![ground; 450],
                &[],
                "an item at rest",
            ),
            (
                &EntityTypeData::FALLING_BLOCK,
                ground,
                vec![ground; 450],
                &[],
                "a falling block at rest",
            ),
        ];
        for (kind, spawn, states, expected, case) in rows {
            let mut tracker = simulated(kind, spawn);
            let ticks: Vec<u64> = updates(&mut tracker, &states)
                .into_iter()
                .map(|(tick, _)| tick)
                .collect();
            assert_eq!(ticks, expected, "{case}");
        }
    }

    #[test]
    fn trk_upd_005_an_item_moves_on_the_linear_path() {
        let states: Vec<MobState> = (0..21)
            .map(|tick| MobState {
                position: DVec3::new(0.5, -40.0 - 0.1 * f64::from(tick), 4.5),
                ..MobState::default()
            })
            .collect();
        let mut tracker = simulated(&EntityTypeData::ITEM, states[0]);
        let updates = updates(&mut tracker, &states);
        assert_eq!(
            updates.last(),
            Some(&(20, linear(states[0].position, states[20].position, false))),
            "one linear move over the interval, not a step of 20 ticks"
        );
    }

    /// A stack of `count` items of the item with the wire id 1 and an empty patch.
    fn stack(count: u8) -> ProtoStack<Wire> {
        ProtoStack::decode(&mut &[count, 1, 0, 0][..]).unwrap()
    }

    #[test]
    fn ien_net_003_a_new_stack_sends_its_entity_data_at_the_next_tracker_tick() {
        let state = MobState {
            position: DVec3::new(0.5, FLOOR, 4.5),
            on_ground: true,
            ..MobState::default()
        };
        let mut tracker = MobTracker::new(state, Entries::Item(stack(3)), &EntityTypeData::ITEM, 0);
        let mut out = BytesMut::new();
        tracker.tick(ID, 0, &mut out).unwrap();
        out.clear();

        tracker.record_stack(stack(5));
        tracker.tick(ID, 1, &mut out).unwrap();

        let expected = record(&ClientboundSetEntityData {
            entity_id: ID,
            metadata: Metadata(vec![MetadataEntry {
                index: 8,
                value: MetaDataValue::Slot(stack(5)),
            }]),
        })
        .unwrap();
        assert_eq!(
            out.freeze(),
            expected,
            "between two periodic updates, the change sends only the new stack"
        );
        let mut out = BytesMut::new();
        tracker.tick(ID, 2, &mut out).unwrap();
        assert!(out.is_empty(), "the stack goes once");
    }
}
