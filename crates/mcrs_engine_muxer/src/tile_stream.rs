use std::collections::BTreeSet;

use crate::entity_id::{EntityId, ViewerId};
use crate::subscription::layer_ranks;
use crate::tier::Tier;

/// The subjects of one tile and the viewers that receive some of them.
#[derive(Debug, Default)]
pub(crate) struct TileStream {
    /// Sorted, so the position of a subject is its rank.
    pub(crate) subjects: Vec<EntityId>,
    pub(crate) viewers: BTreeSet<ViewerId>,
    /// The tiers that have not encoded the last events of the tile yet.
    pub(crate) unsent: [bool; Tier::ALL.len()],
}

impl TileStream {
    pub(crate) fn is_empty(&self) -> bool {
        self.subjects.is_empty() && self.viewers.is_empty()
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
    pub(crate) fn window(&self, layer: usize) -> Option<Window> {
        let ranks = layer_ranks(layer);
        Some(Window {
            first: *self.subjects.get(ranks.start)?,
            after: self.subjects.get(ranks.end).copied(),
        })
    }
}

/// The ids of the subjects of one layer: from `first`, and before `after`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Window {
    first: EntityId,
    after: Option<EntityId>,
}

impl Window {
    pub(crate) fn contains(self, subject: EntityId) -> bool {
        subject >= self.first && self.after.is_none_or(|after| subject < after)
    }
}
