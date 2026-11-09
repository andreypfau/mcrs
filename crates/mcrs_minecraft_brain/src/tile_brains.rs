use alloc::vec::Vec;

use crate::Cadence;
use crate::Decision;
use crate::PlayerRow;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;

/// The brains of the mobs of one tile, in columns, in the order of their order keys.
#[derive(Clone, Debug, PartialEq)]
pub struct TileBrains<B> {
    pub(crate) ids: Vec<EntityId>,
    pub(crate) keys: Vec<EntityKey>,
    pub(crate) cadences: Vec<Cadence>,
    /// The next tick at which each brain runs a full evaluation without a fact.
    pub(crate) wakes: Vec<u64>,
    pub(crate) brains: Vec<B>,
    pub(crate) near_players: Vec<PlayerRow>,
    pub(crate) decisions: Vec<Decision>,
    pub(crate) evaluations: u64,
}

impl<B> Default for TileBrains<B> {
    fn default() -> Self {
        Self {
            ids: Vec::new(),
            keys: Vec::new(),
            cadences: Vec::new(),
            wakes: Vec::new(),
            brains: Vec::new(),
            near_players: Vec::new(),
            decisions: Vec::new(),
            evaluations: 0,
        }
    }
}

/// One brain of a tile, as [`TileBrains::iter`] gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrainEntry<'a, B> {
    pub mob: EntityId,
    pub key: EntityKey,
    pub cadence: Cadence,
    pub wake: u64,
    pub brain: &'a B,
}

impl<B> TileBrains<B> {
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The count of full evaluations that the tile ran.
    #[must_use]
    pub const fn evaluations(&self) -> u64 {
        self.evaluations
    }

    pub fn iter(&self) -> impl Iterator<Item = BrainEntry<'_, B>> {
        self.ids
            .iter()
            .zip(&self.keys)
            .zip(&self.cadences)
            .zip(&self.wakes)
            .zip(&self.brains)
            .map(|((((&mob, &key), &cadence), &wake), brain)| BrainEntry {
                mob,
                key,
                cadence,
                wake,
                brain,
            })
    }

    pub(crate) fn insert(
        &mut self,
        mob: EntityId,
        key: EntityKey,
        cadence: Cadence,
        wake: u64,
        brain: B,
    ) {
        let at = self.keys.partition_point(|other| *other < key);
        self.ids.insert(at, mob);
        self.keys.insert(at, key);
        self.cadences.insert(at, cadence);
        self.wakes.insert(at, wake);
        self.brains.insert(at, brain);
    }

    pub(crate) fn remove(&mut self, key: &EntityKey) -> Option<Removed<B>> {
        let at = self.keys.binary_search(key).ok()?;
        if at >= self.ids.len() {
            return None;
        }
        Some(Removed {
            mob: self.ids.remove(at),
            key: self.keys.remove(at),
            cadence: self.cadences.remove(at),
            wake: self.wakes.remove(at),
            brain: self.brains.remove(at),
        })
    }

    /// Removes the brains of `keys`, which are sorted, in one pass over the columns; gives them in
    /// key order.
    pub(crate) fn remove_all(&mut self, keys: &[EntityKey]) -> Vec<Removed<B>> {
        let leaves: Vec<bool> = self
            .keys
            .iter()
            .map(|key| keys.binary_search(key).is_ok())
            .collect();
        let mut ids = extract(&mut self.ids, &leaves).into_iter();
        let mut cadences = extract(&mut self.cadences, &leaves).into_iter();
        let mut wakes = extract(&mut self.wakes, &leaves).into_iter();
        let mut brains = extract(&mut self.brains, &leaves).into_iter();
        let mut removed = Vec::new();
        for key in extract(&mut self.keys, &leaves) {
            let (Some(mob), Some(cadence), Some(wake), Some(brain)) =
                (ids.next(), cadences.next(), wakes.next(), brains.next())
            else {
                break;
            };
            removed.push(Removed {
                mob,
                key,
                cadence,
                wake,
                brain,
            });
        }
        removed
    }

    /// Adds `entries`, which are sorted by key, in one merge with the columns. The keys of the
    /// tile and of `entries` are distinct.
    pub(crate) fn insert_all(&mut self, entries: Vec<Removed<B>>) {
        if entries.is_empty() {
            return;
        }
        let count = self.ids.len().saturating_add(entries.len());
        let mut merged = TileBrains {
            ids: Vec::with_capacity(count),
            keys: Vec::with_capacity(count),
            cadences: Vec::with_capacity(count),
            wakes: Vec::with_capacity(count),
            brains: Vec::with_capacity(count),
            near_players: core::mem::take(&mut self.near_players),
            decisions: core::mem::take(&mut self.decisions),
            evaluations: self.evaluations,
        };
        let mut held = core::mem::take(self).into_entries().peekable();
        let mut entries = entries.into_iter().peekable();
        loop {
            let next = match (held.peek(), entries.peek()) {
                (Some(old), Some(new)) if new.key < old.key => entries.next(),
                (Some(_), _) => held.next(),
                (None, _) => entries.next(),
            };
            let Some(entry) = next else {
                break;
            };
            merged.ids.push(entry.mob);
            merged.keys.push(entry.key);
            merged.cadences.push(entry.cadence);
            merged.wakes.push(entry.wake);
            merged.brains.push(entry.brain);
        }
        *self = merged;
    }

    fn into_entries(self) -> impl Iterator<Item = Removed<B>> {
        self.ids
            .into_iter()
            .zip(self.keys)
            .zip(self.cadences)
            .zip(self.wakes)
            .zip(self.brains)
            .map(|((((mob, key), cadence), wake), brain)| Removed {
                mob,
                key,
                cadence,
                wake,
                brain,
            })
    }

    pub(crate) fn position(&self, mob: EntityId) -> Option<usize> {
        self.ids.iter().position(|id| *id == mob)
    }
}

pub(crate) struct Removed<B> {
    pub(crate) mob: EntityId,
    pub(crate) key: EntityKey,
    pub(crate) cadence: Cadence,
    pub(crate) wake: u64,
    pub(crate) brain: B,
}

/// Takes the values of `column` whose flag in `leaves` is set, in order; keeps the others.
fn extract<T>(column: &mut Vec<T>, leaves: &[bool]) -> Vec<T> {
    let mut flags = leaves.iter();
    column
        .extract_if(.., |_| flags.next().copied().unwrap_or(false))
        .collect()
}
