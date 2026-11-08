use std::collections::{BTreeMap, BTreeSet};

use bytes::{Bytes, BytesMut};
use mcrs_engine_muxer::{EntityId, ViewerId};
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_protocol::Result;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundBundleDelimiter;
use rustc_hash::FxHashMap;
use uuid::Uuid;

use crate::encode::record;
use crate::mob_bundle::spawn_extras;
use crate::mob_spawn::MobSpawn;
use crate::mob_state::MobState;
use crate::mob_tracker::MobTracker;
use crate::player_codec::wire_id;

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
}

#[derive(Debug)]
struct Mob {
    kind: MobType,
    uuid: Uuid,
    tracker: MobTracker,
    /// The entity data and the attributes of the spawn bundle, as records.
    extras: Bytes,
    /// Sorted.
    holders: Vec<ViewerId>,
}

impl MobTrackers {
    pub(crate) fn contains(&self, subject: EntityId) -> bool {
        self.mobs.contains_key(&subject)
    }

    pub(crate) fn spawn(&mut self, subject: EntityId, spawn: &MobSpawn, tick: u64) -> Result<()> {
        let extras = spawn_extras(wire_id(subject), spawn.kind, &spawn.looks)?;
        self.mobs.insert(
            subject,
            Mob {
                kind: spawn.kind,
                uuid: spawn.uuid,
                tracker: MobTracker::new(spawn.state, spawn.kind.update_interval(), tick),
                extras,
                holders: Vec::new(),
            },
        );
        Ok(())
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
        for viewer in mob.holders {
            if let Some(held) = self.holds.get_mut(&viewer) {
                held.remove(&subject);
            }
        }
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
        let add = record(&mob.tracker.add_entity(
            wire_id(subject),
            mob.uuid,
            mob.kind.entity_type(),
        ))?;
        chunks.extend([delimiter.clone(), add, mob.extras.clone(), delimiter]);
        if let Err(at) = mob.holders.binary_search(&viewer) {
            mob.holders.insert(at, viewer);
        }
        self.held.insert(subject);
        self.holds.entry(viewer).or_default().insert(subject);
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
        true
    }

    /// The viewer no longer subscribes: it holds no mob any more.
    pub(crate) fn forget(&mut self, viewer: ViewerId) {
        for subject in self.holds.remove(&viewer).unwrap_or_default() {
            self.lose(viewer, subject);
        }
    }

    /// Runs the tracker tick of each held mob, and owes the bytes of each update to each viewer
    /// that holds the mob.
    pub(crate) fn finish_tick(&mut self, tick: u64, owe: &mut impl FnMut(ViewerId, Bytes)) {
        let Self {
            mobs,
            held,
            scratch,
            ..
        } = self;
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
                owe(viewer, update.clone());
            }
        }
    }
}
