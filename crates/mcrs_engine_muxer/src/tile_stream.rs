use std::collections::BTreeSet;

use crate::entity_id::{EntityId, ViewerId};

/// The subjects of one tile and the viewers that subscribe to it.
#[derive(Debug, Default)]
pub(crate) struct TileStream {
    pub(crate) subjects: BTreeSet<EntityId>,
    pub(crate) viewers: BTreeSet<ViewerId>,
}

impl TileStream {
    pub(crate) fn is_empty(&self) -> bool {
        self.subjects.is_empty() && self.viewers.is_empty()
    }
}
