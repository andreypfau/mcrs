use alloc::vec::Vec;

use crate::Cadence;
use crate::Decision;
use crate::MobId;
use crate::OrderKey;
use crate::PlayerRow;

/// The brains of the mobs of one tile, in columns, in the order of their order keys.
#[derive(Clone, Debug, PartialEq)]
pub struct TileBrains<B> {
    pub(crate) ids: Vec<MobId>,
    pub(crate) keys: Vec<OrderKey>,
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
    pub mob: MobId,
    pub key: OrderKey,
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
        mob: MobId,
        key: OrderKey,
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

    pub(crate) fn remove(&mut self, key: &OrderKey) -> Option<Removed<B>> {
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

    pub(crate) fn position(&self, mob: MobId) -> Option<usize> {
        self.ids.iter().position(|id| *id == mob)
    }
}

pub(crate) struct Removed<B> {
    pub(crate) mob: MobId,
    pub(crate) key: OrderKey,
    pub(crate) cadence: Cadence,
    pub(crate) wake: u64,
    pub(crate) brain: B,
}
