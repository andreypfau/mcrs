use std::collections::{BTreeMap, BTreeSet};

use bytes::{Bytes, BytesMut};
use mcrs_engine_muxer::{EntityId, ViewerId};
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{ClientboundDamageEvent, ClientboundEntityEvent};
use rustc_hash::FxHashMap;
use uuid::Uuid;

use crate::delivery::Delivery;
use crate::encode::record;
use crate::hold_set::HoldSet;
use crate::mob_bundle::spawn_extras;
use crate::mob_looks::MobLooks;
use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use crate::mob_tracker::MobTracker;
use crate::player_input::Backlog;
use crate::wire_id::wire_id;

const BLOCKS_PER_COLUMN: u32 = 16;

/// The mobs whose latest state a client that caught up receives in one tick.
const RESENDS_PER_TICK: usize = 1024;

/// The mobs of one world. Its updates come at the end of each tick for each held mob, not from
/// the tile blobs (TRK-UPD-009). The [`HoldSet`] of the codec says which client holds each one.
#[derive(Debug, Default)]
pub(crate) struct MobTrackers {
    mobs: FxHashMap<EntityId, Mob>,
    scratch: BytesMut,
    /// The signals of the tick, as records, in the order of their causes.
    signals: Vec<(EntityId, Bytes)>,
    /// The viewers that held each mob that left the world in this tick. They get its signals
    /// before its removal.
    farewells: FxHashMap<EntityId, Vec<ViewerId>>,
    /// The viewers whose connection holds more bytes than their client reads. They get no move
    /// of a mob until they catch up.
    behind: BTreeSet<ViewerId>,
    /// The mobs whose updates each client missed. It gets their latest state once it catches up,
    /// and no relative move before that.
    missed: BTreeMap<ViewerId, BTreeSet<EntityId>>,
}

#[derive(Debug)]
struct Mob {
    kind: &'static EntityTypeData,
    uuid: Uuid,
    looks: MobLooks,
    tracker: MobTracker,
}

impl MobTrackers {
    pub(crate) fn contains(&self, subject: EntityId) -> bool {
        self.mobs.contains_key(&subject)
    }

    pub(crate) fn spawn(&mut self, subject: EntityId, spawn: &MobSpawn, tick: u64) {
        self.mobs.insert(
            subject,
            Mob {
                kind: spawn.kind,
                uuid: spawn.uuid,
                looks: spawn.looks.clone(),
                tracker: MobTracker::new(spawn.state, spawn.kind.update_interval, tick),
            },
        );
    }

    pub(crate) fn set_backlog(&mut self, viewer: ViewerId, backlog: Backlog) {
        match backlog {
            Backlog::Behind => self.behind.insert(viewer),
            Backlog::Clear => self.behind.remove(&viewer),
        };
    }

    /// Queues `signal` of `subject` for the clients that hold it at the end of the tick.
    pub(crate) fn signal(
        &mut self,
        subject: EntityId,
        signal: MobSignal,
        holds: &HoldSet,
    ) -> Result<()> {
        if holds.holders(subject).is_empty() {
            return Ok(());
        }
        let entity_id = wire_id(subject);
        let event = |entity_status| {
            record(&ClientboundEntityEvent {
                entity_id: entity_id.0,
                entity_status,
            })
        };
        let packet = match signal {
            MobSignal::Hurt { damage_type } => {
                let Ok(damage_type) = i32::try_from(damage_type) else {
                    tracing::warn!("the damage type {damage_type} of a hit has no id on the wire");
                    return Ok(());
                };
                record(&ClientboundDamageEvent {
                    entity_id,
                    damage_type: VarInt(damage_type),
                    cause_entity_id: VarInt(0),
                    direct_entity_id: VarInt(0),
                    source_position: None,
                })?
            }
            MobSignal::Died => event(ClientboundEntityEvent::DIED)?,
            MobSignal::Removed => event(ClientboundEntityEvent::REMOVED)?,
        };
        self.signals.push((subject, packet));
        Ok(())
    }

    /// Appends the signals of the tick of each mob of `subjects` that left the world, for the
    /// client of `viewer`, before the removal of the mobs.
    pub(crate) fn farewell(
        &self,
        viewer: ViewerId,
        subjects: &[EntityId],
        chunks: &mut Vec<Bytes>,
    ) {
        for (subject, packet) in &self.signals {
            let held = self
                .farewells
                .get(subject)
                .is_some_and(|holders| holders.contains(&viewer));
            if held && subjects.contains(subject) {
                chunks.push(packet.clone());
            }
        }
    }

    pub(crate) fn record(&mut self, subject: EntityId, state: MobState) {
        if let Some(mob) = self.mobs.get_mut(&subject) {
            mob.tracker.record(state);
        }
    }

    /// Forgets `subject` and takes it out of `holds`. The muxer removes it from each client that
    /// holds it.
    pub(crate) fn despawn(&mut self, subject: EntityId, holds: &mut HoldSet) {
        if self.mobs.remove(&subject).is_none() {
            return;
        }
        let holders = holds.remove_subject(subject);
        for viewer in &holders {
            if let Some(missed) = self.missed.get_mut(viewer) {
                missed.remove(&subject);
            }
        }
        self.farewells.insert(subject, holders);
    }

    /// The tracking range of `subject` in blocks (TRK-RNG-001), at the
    /// `entity-broadcast-range-percentage` of 100. The view distance of the viewer does not limit
    /// it (TRK-DEV-004).
    pub(crate) fn tracking_range(&self, subject: EntityId) -> Option<u32> {
        let mob = self.mobs.get(&subject)?;
        Some(mob.kind.tracking_range.saturating_mul(BLOCKS_PER_COLUMN))
    }

    pub(crate) fn position(&self, subject: EntityId) -> Option<[f64; 3]> {
        let mob = self.mobs.get(&subject)?;
        Some(mob.tracker.latest().position.to_array())
    }

    /// Appends the spawn bundle of `subject` for the client of `viewer` (TRK-SPN-001), at tick
    /// `tick`, between two `delimiter`s, and records the hold in `holds`.
    pub(crate) fn gain(
        &mut self,
        viewer: ViewerId,
        subject: EntityId,
        tick: u64,
        delimiter: &Bytes,
        holds: &mut HoldSet,
        chunks: &mut Vec<Bytes>,
    ) -> Result<()> {
        let Some(mob) = self.mobs.get_mut(&subject) else {
            return Ok(());
        };
        if holds.holders(subject).is_empty() && mob.tracker.is_stale(tick) {
            mob.tracker.restart();
        }
        let id = wire_id(subject);
        let add = record(&mob.tracker.add_entity(id, mob.uuid, mob.kind.entity_type))?;
        let extras = spawn_extras(id, mob.kind, &mob.looks, mob.tracker.sent_entries())?;
        chunks.extend([delimiter.clone(), add, extras, delimiter.clone()]);
        holds.insert(viewer, subject);
        if let Some(missed) = self.missed.get_mut(&viewer) {
            missed.remove(&subject);
        }
        Ok(())
    }

    /// Ends what the client of `viewer` missed of `subject`, which it no longer holds; false when
    /// `subject` is not a mob.
    pub(crate) fn lose(&mut self, viewer: ViewerId, subject: EntityId) -> bool {
        if !self.mobs.contains_key(&subject) {
            return false;
        }
        if let Some(missed) = self.missed.get_mut(&viewer) {
            missed.remove(&subject);
        }
        true
    }

    /// The viewer no longer subscribes: it holds no mob any more.
    pub(crate) fn forget(&mut self, viewer: ViewerId, holds: &mut HoldSet) {
        holds.release_where(viewer, |subject| self.mobs.contains_key(&subject));
        self.behind.remove(&viewer);
        self.missed.remove(&viewer);
    }

    /// Runs the tracker tick of each held mob, and owes the bytes of each update to each viewer
    /// that holds the mob. A viewer that is behind misses the update; a viewer that caught up
    /// gets the latest state of the mobs it missed, a share in each tick.
    pub(crate) fn finish_tick(&mut self, tick: u64, holds: &HoldSet, delivery: &mut Delivery) {
        let Self {
            mobs,
            scratch,
            signals,
            farewells,
            behind,
            missed,
        } = self;
        for (subject, packet) in signals.drain(..) {
            for &viewer in holds.holders(subject) {
                delivery.owe(viewer, packet.clone());
            }
        }
        farewells.clear();
        for (subject, holders) in holds.held() {
            let Some(mob) = mobs.get_mut(&subject) else {
                continue;
            };
            scratch.clear();
            if let Err(error) = mob.tracker.tick(wire_id(subject), tick, scratch) {
                tracing::warn!("the update of a mob did not encode: {error:#}");
                continue;
            }
            if scratch.is_empty() {
                continue;
            }
            let update = scratch.split().freeze();
            for &viewer in holders {
                let misses = behind.contains(&viewer)
                    || missed
                        .get(&viewer)
                        .is_some_and(|missed| missed.contains(&subject));
                if misses {
                    missed.entry(viewer).or_default().insert(subject);
                } else {
                    delivery.owe(viewer, update.clone());
                }
            }
        }
        resend_missed(mobs, behind, missed, scratch, delivery);
    }
}

/// Owes each viewer that is not behind the latest state of some of the mobs it missed.
fn resend_missed(
    mobs: &FxHashMap<EntityId, Mob>,
    behind: &BTreeSet<ViewerId>,
    missed: &mut BTreeMap<ViewerId, BTreeSet<EntityId>>,
    scratch: &mut BytesMut,
    delivery: &mut Delivery,
) {
    for (viewer, subjects) in missed.iter_mut() {
        if behind.contains(viewer) {
            continue;
        }
        scratch.clear();
        for _ in 0..RESENDS_PER_TICK {
            let Some(subject) = subjects.pop_first() else {
                break;
            };
            let Some(mob) = mobs.get(&subject) else {
                continue;
            };
            if let Err(error) = mob.tracker.resend(wire_id(subject), scratch) {
                tracing::warn!("the state of a mob did not encode: {error:#}");
            }
        }
        if !scratch.is_empty() {
            delivery.owe(*viewer, scratch.split().freeze());
        }
    }
    missed.retain(|_, subjects| !subjects.is_empty());
}

#[cfg(test)]
mod tests {
    use glam::DVec3;
    use mcrs_engine_muxer::ViewerBytes;
    use mcrs_minecraft_core::Look;
    use mcrs_minecraft_protocol_game::clientbound::{ClientboundEntityPositionSync, PositionPath};

    use super::*;

    const MOB: EntityId = EntityId(8);
    const KEEPS_UP: ViewerId = ViewerId(1);
    const BEHIND: ViewerId = ViewerId(2);

    fn at(x: f64) -> MobState {
        MobState {
            position: DVec3::new(x, -60.0, 0.5),
            on_ground: true,
            health: 10.0,
            ..MobState::default()
        }
    }

    fn sync_at(x: f64) -> Bytes {
        record(&ClientboundEntityPositionSync {
            entity_id: wire_id(MOB),
            position: PositionPath::Linear(at(x).position),
            look: Look {
                yaw: 0.0,
                pitch: 0.0,
            },
            on_ground: true,
        })
        .unwrap()
    }

    #[derive(Default)]
    struct World {
        mobs: MobTrackers,
        holds: HoldSet,
    }

    impl World {
        fn spawned(state: MobState) -> Self {
            let mut world = Self::default();
            let spawn = MobSpawn {
                kind: &EntityTypeData::PIG,
                uuid: Uuid::nil(),
                state,
                looks: MobLooks::default(),
            };
            world.mobs.spawn(MOB, &spawn, 0);
            world
        }

        fn gain(&mut self, viewer: ViewerId, tick: u64) -> Vec<Bytes> {
            let mut chunks = Vec::new();
            let delimiter = Bytes::new();
            self.mobs
                .gain(viewer, MOB, tick, &delimiter, &mut self.holds, &mut chunks)
                .unwrap();
            chunks
        }

        fn tick(&mut self, tick: u64) -> BTreeMap<ViewerId, Vec<Bytes>> {
            let mut delivery = Delivery::default();
            self.mobs.finish_tick(tick, &self.holds, &mut delivery);
            let mut out = Vec::new();
            delivery.drain(&mut out);
            out.into_iter()
                .map(|ViewerBytes { viewer, chunks }| (viewer, chunks))
                .collect()
        }
    }

    #[test]
    fn a_viewer_that_is_behind_misses_the_moves_and_then_gets_the_state_the_others_hold() {
        let mut world = World::spawned(at(0.5));
        for viewer in [KEEPS_UP, BEHIND] {
            world.gain(viewer, 0);
        }
        world.tick(0);

        world.mobs.set_backlog(BEHIND, Backlog::Behind);
        let mut moves = 0;
        for (step, now) in (1..=6).enumerate() {
            world.mobs.record(MOB, at(0.5 + 0.2 * (step + 1) as f64));
            let owed = world.tick(now);
            assert!(!owed.contains_key(&BEHIND), "no move while behind");
            moves += owed.get(&KEEPS_UP).map_or(0, Vec::len);
        }
        assert!(moves > 0, "the other viewer keeps getting the moves");

        world.mobs.set_backlog(BEHIND, Backlog::Clear);
        let owed = world.tick(7);
        let resent = owed.get(&BEHIND).and_then(|chunks| chunks.first()).unwrap();
        assert!(
            resent.starts_with(&sync_at(0.5 + 0.2 * 6.0)),
            "the latest state that the other viewer holds, as an absolute position"
        );
        assert!(!world.tick(8).contains_key(&BEHIND), "sent once");
    }

    #[test]
    fn a_mob_that_leaves_while_a_viewer_is_behind_is_not_resent() {
        let mut world = World::spawned(at(0.5));
        world.gain(BEHIND, 0);
        world.tick(0);
        world.mobs.set_backlog(BEHIND, Backlog::Behind);
        world.mobs.record(MOB, at(2.5));
        world.tick(3);

        world.mobs.despawn(MOB, &mut world.holds);
        world.mobs.set_backlog(BEHIND, Backlog::Clear);

        assert!(
            world.holds.holders(MOB).is_empty(),
            "no client holds a mob that left"
        );
        assert!(
            !world.tick(4).contains_key(&BEHIND),
            "the state of a mob that left is not resent"
        );
    }

    #[test]
    fn the_first_viewer_after_a_gap_gets_the_latest_state_in_the_spawn() {
        let mut world = World::spawned(at(0.5));
        world.gain(KEEPS_UP, 0);
        world.tick(0);
        world.holds.remove(KEEPS_UP, MOB);
        world.mobs.lose(KEEPS_UP, MOB);
        for now in 1..10_u32 {
            world.mobs.record(MOB, at(0.5 + f64::from(now)));
            world.tick(u64::from(now));
        }

        world.gain(BEHIND, 10);

        let add = world.mobs.mobs[&MOB].tracker.add_entity(
            wire_id(MOB),
            Uuid::nil(),
            EntityTypeData::PIG.entity_type,
        );
        assert_eq!(
            add.pos,
            at(9.5).position,
            "the spawn shows the last state, not the one that no client received"
        );
    }
}
