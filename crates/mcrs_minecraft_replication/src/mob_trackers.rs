use std::collections::{BTreeMap, BTreeSet};

use bytes::{Bytes, BytesMut};
use mcrs_engine_muxer::{EntityId, ViewerId};
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundBundleDelimiter;
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{ClientboundDamageEvent, ClientboundEntityEvent};
use rustc_hash::FxHashMap;
use uuid::Uuid;

use crate::encode::record;
use crate::mob_bundle::spawn_extras;
use crate::mob_looks::MobLooks;
use crate::mob_signal::MobSignal;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use crate::mob_tracker::MobTracker;
use crate::player_codec::wire_id;
use crate::player_input::Backlog;

const BLOCKS_PER_COLUMN: u32 = 16;

/// The mobs whose latest state a client that caught up receives in one tick.
const RESENDS_PER_TICK: usize = 1024;

/// The mobs of one world and the viewers whose client holds each one. Its updates come at the
/// end of each tick for each held mob, not from the tile blobs (TRK-UPD-009).
#[derive(Debug, Default)]
pub(crate) struct MobTrackers {
    mobs: FxHashMap<EntityId, Mob>,
    /// The mobs that some client holds, in the order of their ids.
    held: BTreeSet<EntityId>,
    /// The mobs that each client holds.
    holds: BTreeMap<ViewerId, BTreeSet<EntityId>>,
    delimiter: Option<Bytes>,
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
    kind: MobType,
    uuid: Uuid,
    looks: MobLooks,
    tracker: MobTracker,
    /// Sorted.
    holders: Vec<ViewerId>,
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
                looks: spawn.looks,
                tracker: MobTracker::new(spawn.state, spawn.kind.update_interval(), tick),
                holders: Vec::new(),
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
    pub(crate) fn signal(&mut self, subject: EntityId, signal: MobSignal) -> Result<()> {
        if !self.held.contains(&subject) {
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
            MobSignal::Hurt { damage_type } => record(&ClientboundDamageEvent {
                entity_id,
                damage_type: VarInt(i32::try_from(damage_type).unwrap_or(i32::MAX)),
                cause_entity_id: VarInt(0),
                direct_entity_id: VarInt(0),
                source_position: None,
            })?,
            MobSignal::Died => event(3)?,
            MobSignal::Removed => event(60)?,
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

    /// Forgets `subject`. The muxer removes it from each client that holds it.
    pub(crate) fn despawn(&mut self, subject: EntityId) {
        let Some(mob) = self.mobs.remove(&subject) else {
            return;
        };
        self.held.remove(&subject);
        self.farewells.insert(subject, mob.holders.clone());
        for viewer in mob.holders {
            if let Some(held) = self.holds.get_mut(&viewer) {
                held.remove(&subject);
            }
            if let Some(missed) = self.missed.get_mut(&viewer) {
                missed.remove(&subject);
            }
        }
    }

    /// The tracking range of `subject` in blocks (TRK-RNG-001), at the
    /// `entity-broadcast-range-percentage` of 100. The view distance of the viewer does not limit
    /// it (TRK-DEV-004).
    pub(crate) fn tracking_range(&self, subject: EntityId) -> Option<u32> {
        let mob = self.mobs.get(&subject)?;
        Some(mob.kind.tracking_range().saturating_mul(BLOCKS_PER_COLUMN))
    }

    pub(crate) fn position(&self, subject: EntityId) -> Option<[f64; 3]> {
        let mob = self.mobs.get(&subject)?;
        Some(mob.tracker.latest().position.to_array())
    }

    /// Appends the spawn bundle of `subject` for the client of `viewer` (TRK-SPN-001), at tick
    /// `tick`.
    pub(crate) fn gain(
        &mut self,
        viewer: ViewerId,
        subject: EntityId,
        tick: u64,
        chunks: &mut Vec<Bytes>,
    ) -> Result<()> {
        let delimiter = match &self.delimiter {
            Some(delimiter) => delimiter.clone(),
            None => self
                .delimiter
                .insert(record(&ClientboundBundleDelimiter)?)
                .clone(),
        };
        let Some(mob) = self.mobs.get_mut(&subject) else {
            return Ok(());
        };
        if mob.holders.is_empty() && mob.tracker.is_stale(tick) {
            mob.tracker.restart();
        }
        let id = wire_id(subject);
        let add = record(&mob.tracker.add_entity(id, mob.uuid, mob.kind.entity_type()))?;
        let (health, dying) = mob.tracker.sent_data();
        let extras = spawn_extras(id, mob.kind, &mob.looks, health, dying)?;
        chunks.extend([delimiter.clone(), add, extras, delimiter]);
        if let Err(at) = mob.holders.binary_search(&viewer) {
            mob.holders.insert(at, viewer);
        }
        self.held.insert(subject);
        self.holds.entry(viewer).or_default().insert(subject);
        if let Some(missed) = self.missed.get_mut(&viewer) {
            missed.remove(&subject);
        }
        Ok(())
    }

    /// Takes `subject` out of the client of `viewer`; false when it is not a mob.
    pub(crate) fn lose(&mut self, viewer: ViewerId, subject: EntityId) -> bool {
        let Some(mob) = self.mobs.get_mut(&subject) else {
            return false;
        };
        if let Ok(at) = mob.holders.binary_search(&viewer) {
            mob.holders.remove(at);
        }
        if mob.holders.is_empty() {
            self.held.remove(&subject);
        }
        if let Some(held) = self.holds.get_mut(&viewer) {
            held.remove(&subject);
        }
        if let Some(missed) = self.missed.get_mut(&viewer) {
            missed.remove(&subject);
        }
        true
    }

    /// The viewer no longer subscribes: it holds no mob any more.
    pub(crate) fn forget(&mut self, viewer: ViewerId) {
        for subject in self.holds.remove(&viewer).unwrap_or_default() {
            self.lose(viewer, subject);
        }
        self.behind.remove(&viewer);
        self.missed.remove(&viewer);
    }

    /// Runs the tracker tick of each held mob, and owes the bytes of each update to each viewer
    /// that holds the mob. A viewer that is behind misses the update; a viewer that caught up
    /// gets the latest state of the mobs it missed, a share in each tick.
    pub(crate) fn finish_tick(&mut self, tick: u64, owe: &mut impl FnMut(ViewerId, Bytes)) {
        let Self {
            mobs,
            held,
            scratch,
            signals,
            farewells,
            behind,
            missed,
            ..
        } = self;
        for (subject, packet) in signals.drain(..) {
            if let Some(mob) = mobs.get(&subject) {
                for &viewer in &mob.holders {
                    owe(viewer, packet.clone());
                }
            }
        }
        farewells.clear();
        for subject in held.iter() {
            let Some(mob) = mobs.get_mut(subject) else {
                continue;
            };
            scratch.clear();
            if let Err(error) = mob.tracker.tick(wire_id(*subject), tick, scratch) {
                tracing::warn!("the update of a mob did not encode: {error:#}");
                continue;
            }
            if scratch.is_empty() {
                continue;
            }
            let update = scratch.split().freeze();
            for &viewer in &mob.holders {
                let misses = behind.contains(&viewer)
                    || missed
                        .get(&viewer)
                        .is_some_and(|missed| missed.contains(subject));
                if misses {
                    missed.entry(viewer).or_default().insert(*subject);
                } else {
                    owe(viewer, update.clone());
                }
            }
        }
        resend_missed(mobs, behind, missed, scratch, owe);
    }
}

/// Owes each viewer that is not behind the latest state of some of the mobs it missed.
fn resend_missed(
    mobs: &FxHashMap<EntityId, Mob>,
    behind: &BTreeSet<ViewerId>,
    missed: &mut BTreeMap<ViewerId, BTreeSet<EntityId>>,
    scratch: &mut BytesMut,
    owe: &mut impl FnMut(ViewerId, Bytes),
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
            owe(*viewer, scratch.split().freeze());
        }
    }
    missed.retain(|_, subjects| !subjects.is_empty());
}

#[cfg(test)]
mod tests {
    use glam::DVec3;
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

    fn tick(mobs: &mut MobTrackers, tick: u64) -> BTreeMap<ViewerId, Vec<Bytes>> {
        let mut owed: BTreeMap<ViewerId, Vec<Bytes>> = BTreeMap::new();
        mobs.finish_tick(tick, &mut |viewer, bytes| {
            owed.entry(viewer).or_default().push(bytes);
        });
        owed
    }

    #[test]
    fn a_viewer_that_is_behind_misses_the_moves_and_then_gets_the_state_the_others_hold() {
        let mut mobs = MobTrackers::default();
        let spawn = MobSpawn {
            kind: MobType::Pig,
            uuid: Uuid::nil(),
            state: at(0.5),
            looks: MobLooks::default(),
        };
        mobs.spawn(MOB, &spawn, 0);
        for viewer in [KEEPS_UP, BEHIND] {
            mobs.gain(viewer, MOB, 0, &mut Vec::new()).unwrap();
        }
        tick(&mut mobs, 0);

        mobs.set_backlog(BEHIND, Backlog::Behind);
        let mut moves = 0;
        for (step, now) in (1..=6).enumerate() {
            mobs.record(MOB, at(0.5 + 0.2 * (step + 1) as f64));
            let owed = tick(&mut mobs, now);
            assert!(!owed.contains_key(&BEHIND), "no move while behind");
            moves += owed.get(&KEEPS_UP).map_or(0, Vec::len);
        }
        assert!(moves > 0, "the other viewer keeps getting the moves");

        mobs.set_backlog(BEHIND, Backlog::Clear);
        let owed = tick(&mut mobs, 7);
        let expected = record(&ClientboundEntityPositionSync {
            entity_id: wire_id(MOB),
            position: PositionPath::Linear(at(0.5 + 0.2 * 6.0).position),
            look: Look {
                yaw: 0.0,
                pitch: 0.0,
            },
            on_ground: true,
        })
        .unwrap();
        let resent = owed.get(&BEHIND).and_then(|chunks| chunks.first()).unwrap();
        assert!(
            resent.starts_with(&expected),
            "the latest state that the other viewer holds, as an absolute position"
        );
        assert!(!tick(&mut mobs, 8).contains_key(&BEHIND), "sent once");
    }
}
