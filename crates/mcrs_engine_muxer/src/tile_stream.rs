use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use crate::entity_id::ViewerId;
use crate::subscription::layer_ranks;
use crate::tier::Tier;
use mcrs_minecraft_entity_key::EntityId;

/// The subjects of one tile and the viewers that receive some of them.
#[derive(Debug, Default)]
pub(crate) struct TileStream {
    /// The subjects that the planner places, sorted, so the position of a subject is its rank.
    pub(crate) subjects: Vec<EntityId>,
    pub(crate) viewers: BTreeSet<ViewerId>,
    /// The subjects with a tracking range, sorted by subject.
    pub(crate) ranged: Vec<RangedSubject>,
    /// The viewers whose tracking region holds the tile.
    pub(crate) watchers: BTreeSet<ViewerId>,
    /// The tiers that have not encoded the last events of the tile yet.
    pub(crate) unsent: [bool; Tier::ALL.len()],
}

impl TileStream {
    pub(crate) fn is_empty(&self) -> bool {
        self.subjects.is_empty()
            && self.viewers.is_empty()
            && self.ranged.is_empty()
            && self.watchers.is_empty()
    }

    pub(crate) fn insert_ranged(&mut self, subject: EntityId, range: u32) {
        match self.ranged_at(subject) {
            Ok(at) => {
                if let Some(entry) = self.ranged.get_mut(at) {
                    entry.range = range;
                }
            }
            Err(at) => self.ranged.insert(at, RangedSubject { subject, range }),
        }
    }

    pub(crate) fn remove_ranged(&mut self, subject: EntityId) {
        if let Ok(at) = self.ranged_at(subject) {
            self.ranged.remove(at);
        }
    }

    /// The tracking range of `subject` when the tile holds it as a ranged subject.
    pub(crate) fn range_of(&self, subject: EntityId) -> Option<u32> {
        let at = self.ranged_at(subject).ok()?;
        self.ranged.get(at).map(|entry| entry.range)
    }

    fn ranged_at(&self, subject: EntityId) -> Result<usize, usize> {
        self.ranged
            .binary_search_by_key(&subject, |entry| entry.subject)
    }

    pub(crate) fn insert(&mut self, subject: EntityId) {
        if let Err(rank) = self.subjects.binary_search(&subject) {
            self.subjects.insert(rank, subject);
        }
    }

    pub(crate) fn remove(&mut self, subject: EntityId) {
        if let Ok(rank) = self.subjects.binary_search(&subject) {
            self.subjects.remove(rank);
        }
    }

    pub(crate) fn rank(&self, subject: EntityId) -> Option<usize> {
        self.subjects.binary_search(&subject).ok()
    }

    /// The subjects of `layer`.
    pub(crate) fn layer(&self, layer: usize) -> &[EntityId] {
        let ranks = layer_ranks(layer);
        let rest = self.subjects.get(ranks.start..).unwrap_or_default();
        rest.get(..ranks.len()).unwrap_or(rest)
    }

    /// The subjects of `layer` as a range of ids, or `None` when the layer is empty.
    pub(crate) fn window(&self, layer: usize) -> Option<RangeInclusive<EntityId>> {
        let members = self.layer(layer);
        Some(*members.first()?..=*members.last()?)
    }
}

/// A subject that a viewer holds while it is within `range` of the viewer, in the units of the
/// game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RangedSubject {
    pub(crate) subject: EntityId,
    pub(crate) range: u32,
}
