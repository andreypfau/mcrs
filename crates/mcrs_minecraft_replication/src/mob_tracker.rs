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
use mcrs_minecraft_protocol_game::entity::{Metadata, MetadataEntry};
use mcrs_minecraft_protocol_game::{ByteAngle, LpVec3, position_delta};
use uuid::Uuid;

use crate::delta_base::all_or_nothing;
use crate::mob_bundle::{Entries, LivingEntries};
use crate::mob_state::MobState;
use crate::move_choice::{Due, MOVED, MoveChoice, POSITION_REFRESH, Resync, UPDATES_PER_SYNC};
use crate::subject_attributes::SubjectAttributes;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;

const VELOCITY_CHANGE: f64 = 1.0e-7;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MobTracker {
    kind: &'static EntityTypeData,
    start: u64,
    last: u64,
    latest: MobState,
    entries: Entries,
    sent_entries: Entries,
    sent: Sent,
    since_sync: u32,
    open: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Sent {
    position: DVec3,
    velocity: DVec3,
    yaw: ByteAngle,
    pitch: ByteAngle,
    head_yaw: ByteAngle,
    on_ground: bool,
}

impl Sent {
    fn of(state: &MobState) -> Self {
        Self {
            position: state.position,
            velocity: state.velocity,
            yaw: ByteAngle::from_degrees(state.yaw),
            pitch: ByteAngle::from_degrees(state.pitch),
            head_yaw: ByteAngle::from_degrees(state.head_yaw),
            on_ground: state.on_ground,
        }
    }
}

impl MobTracker {
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
            sent: Sent::of(&state),
            sent_entries: entries.clone(),
            entries,
            since_sync: 0,
            open: 0,
        }
    }

    pub(crate) fn latest(&self) -> &MobState {
        &self.latest
    }

    pub(crate) fn sent_entries(&self) -> &Entries {
        &self.sent_entries
    }

    pub(crate) fn record(&mut self, state: MobState) {
        self.latest = state;
        if let Entries::Living(living) = &mut self.entries {
            *living = LivingEntries::of(&state);
        }
    }

    pub(crate) fn record_stack(&mut self, stack: ProtoStack<Wire>) {
        if let Entries::Item(item) = &mut self.entries {
            *item = stack;
        }
    }

    pub(crate) fn record_entries(&mut self, entries: &[MetadataEntry<'static>]) {
        self.entries.merge(entries);
    }

    pub(crate) fn is_stale(&self, tick: u64) -> bool {
        tick.wrapping_sub(self.last) > 1
    }

    pub(crate) fn restart(&mut self) {
        self.sent = Sent::of(&self.latest);
        self.sent_entries = self.entries.clone();
        self.since_sync = 0;
        self.open = 0;
    }

    fn dirty(&self) -> bool {
        self.entries != self.sent_entries
    }

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

    pub(crate) fn tick(
        &mut self,
        id: VarInt,
        tick: u64,
        attributes: &mut SubjectAttributes,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let interval = u64::from(self.kind.update_interval.max(1));
        let start = if self.latest.bounced && !self.kind.is_living() {
            let count = tick.wrapping_sub(self.start);
            let jumped = (count / interval).wrapping_add(1).wrapping_mul(interval);
            tick.wrapping_sub(jumped)
        } else {
            self.start
        };
        let count = tick.wrapping_sub(start);
        let dirty = self.dirty();
        if count.is_multiple_of(interval) || dirty {
            let (sent, since_sync) = (self.sent, self.since_sync);
            if let Err(error) = all_or_nothing(bytes, |bytes| {
                self.update(id, count, dirty, attributes, bytes)
            }) {
                self.sent = sent;
                self.since_sync = since_sync;
                return Err(error);
            }
            if dirty {
                self.sent_entries = self.entries.clone();
            }
            attributes.mark_sent();
        }
        self.start = start;
        self.latest.bounced = false;
        self.open = self.open.saturating_add(1);
        self.last = tick;
        Ok(())
    }

    fn update(
        &mut self,
        id: VarInt,
        count: u64,
        dirty: bool,
        attributes: &SubjectAttributes,
        bytes: &mut BytesMut,
    ) -> Result<()> {
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
        if dirty {
            write_record(
                bytes,
                &ClientboundSetEntityData {
                    entity_id: id,
                    metadata: Metadata(self.entries.changed_from(&self.sent_entries)),
                },
            )?;
        }
        attributes.write_unsent(id, bytes)?;
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

    fn open_ticks(&self) -> u32 {
        if self.kind.is_living() { self.open } else { 0 }
    }

    pub(crate) fn resend(
        &self,
        id: VarInt,
        attributes: &SubjectAttributes,
        bytes: &mut BytesMut,
    ) -> Result<()> {
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
                    metadata: Metadata(self.sent_entries.all()),
                },
            )?;
            attributes.write_all(id, bytes)
        })
    }

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
            tracker
                .tick(ID, tick, &mut SubjectAttributes::default(), &mut out)
                .unwrap();
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

    fn simulated(kind: &'static EntityTypeData, spawn: MobState) -> MobTracker {
        MobTracker::new(
            spawn,
            Entries::FallingBlock(BlockPos::new(0, 0, 0)),
            kind,
            0,
        )
    }

    fn updates(tracker: &mut MobTracker, states: &[MobState]) -> Vec<(u64, Bytes)> {
        let mut updates = Vec::new();
        for (tick, state) in (0..).zip(states) {
            tracker.record(*state);
            let mut out = BytesMut::new();
            tracker
                .tick(ID, tick, &mut SubjectAttributes::default(), &mut out)
                .unwrap();
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
    fn trk_upd_003_016_the_401st_counted_update_resyncs() {
        let path = |tick: u64, jump: u64, rest: core::ops::RangeInclusive<u64>| {
            let moving = if tick > *rest.end() {
                tick - (rest.end() - rest.start())
            } else {
                tick.min(*rest.start())
            };
            let x = if tick >= jump { 100.5 } else { 0.5 };
            DVec3::new(x, -40.0 + 0.01 * moving as f64, 4.5)
        };
        let rows: [(
            &'static EntityTypeData,
            u64,
            core::ops::RangeInclusive<u64>,
            u64,
            [u64; 2],
            &str,
        ); 2] = [
            (
                &EntityTypeData::ZOMBIFIED_PIGLIN,
                297,
                u64::MAX..=u64::MAX,
                1210,
                [297, 1200],
                "a mob: update n at tick 3 (n - 1)",
            ),
            (
                &EntityTypeData::ITEM,
                1000,
                2000..=4000,
                10010,
                [1000, 10000],
                "an item: 101 counted updates to tick 2000, none at rest to tick 4000",
            ),
        ];
        for (kind, jump, rest, ticks, expected, case) in rows {
            let states: Vec<MobState> = (0..ticks)
                .map(|tick| MobState {
                    position: path(tick, jump, rest.clone()),
                    ..MobState::default()
                })
                .collect();
            let spawn = MobState {
                position: DVec3::new(0.5, -41.0, 4.5),
                ..MobState::default()
            };
            let mut tracker = MobTracker::new(
                spawn,
                Entries::FallingBlock(BlockPos::new(0, 0, 0)),
                kind,
                0,
            );
            let syncs: Vec<u64> = updates(&mut tracker, &states)
                .into_iter()
                .filter(|(tick, bytes)| {
                    let position = states[*tick as usize].position;
                    let path = if kind.is_living() && *tick > 0 {
                        PositionPath::Stepped {
                            steps: vec![PositionStep {
                                position,
                                tick_offset: VarInt(3),
                            }],
                            start_immediately: false,
                        }
                    } else {
                        PositionPath::Linear(position)
                    };
                    let sync = record(&ClientboundEntityPositionSync {
                        entity_id: ID,
                        position: path,
                        look: Look::default(),
                        on_ground: false,
                    })
                    .unwrap();
                    *bytes == sync
                })
                .map(|(tick, _)| tick)
                .collect();
            assert_eq!(syncs, expected, "{case}");
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

    #[test]
    fn an_update_that_fails_to_encode_leaves_the_tracker_as_it_was() {
        let spawn = MobState {
            position: DVec3::new(0.5, -40.0, 4.5),
            ..MobState::default()
        };
        let mut tracker = MobTracker::new(spawn, Entries::custom(&[]), &EntityTypeData::ITEM, 0);
        tracker
            .tick(
                ID,
                0,
                &mut SubjectAttributes::default(),
                &mut BytesMut::new(),
            )
            .unwrap();
        tracker.record(MobState {
            position: DVec3::new(0.5, -41.0, 4.5),
            bounced: true,
            ..MobState::default()
        });
        tracker.record_entries(&[MetadataEntry {
            index: 0xFF,
            value: MetaDataValue::Byte(0),
        }]);
        let before = tracker.clone();
        let mut out = BytesMut::new();

        let result = tracker.tick(ID, 5, &mut SubjectAttributes::default(), &mut out);

        assert!(
            result.is_err(),
            "the index 0xFF is the end marker of entity data"
        );
        assert!(out.is_empty(), "no packet of the update is written");
        assert_eq!(
            tracker, before,
            "the bounce and the move wait for the next tick"
        );
    }

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
        tracker
            .tick(ID, 0, &mut SubjectAttributes::default(), &mut out)
            .unwrap();
        out.clear();

        tracker.record_stack(stack(5));
        tracker
            .tick(ID, 1, &mut SubjectAttributes::default(), &mut out)
            .unwrap();

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
        tracker
            .tick(ID, 2, &mut SubjectAttributes::default(), &mut out)
            .unwrap();
        assert!(out.is_empty(), "the stack goes once");
    }
}
