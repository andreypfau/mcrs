use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;

use crate::entity_id::{EntityId, ViewerId};

/// The subjects of one tile and the viewers that subscribe to it.
#[derive(Debug, Default)]
pub(crate) struct TileStream {
    /// Each subject with its spawn bundle, cached until the next event of the subject.
    pub(crate) subjects: BTreeMap<EntityId, Option<Bytes>>,
    pub(crate) viewers: BTreeSet<ViewerId>,
}

impl TileStream {
    pub(crate) fn is_empty(&self) -> bool {
        self.subjects.is_empty() && self.viewers.is_empty()
    }
}
