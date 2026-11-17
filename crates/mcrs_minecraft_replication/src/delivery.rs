use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use bytes::Bytes;
use mcrs_engine_muxer::{ViewerBytes, ViewerId};

#[derive(Debug, Default)]
pub(crate) struct Delivery {
    owed: BTreeMap<ViewerId, Vec<Bytes>>,
    forgotten: BTreeSet<ViewerId>,
}

impl Delivery {
    pub(crate) fn owe(&mut self, viewer: ViewerId, packet: Bytes) {
        if !self.forgotten.contains(&viewer) {
            self.owed.entry(viewer).or_default().push(packet);
        }
    }

    pub(crate) fn drop_owed(&mut self, viewer: ViewerId) {
        self.owed.remove(&viewer);
    }

    pub(crate) fn forget(&mut self, viewer: ViewerId) {
        self.forgotten.insert(viewer);
    }

    pub(crate) fn remember(&mut self, viewer: ViewerId) {
        self.forgotten.remove(&viewer);
    }

    pub(crate) fn drain(&mut self, out: &mut Vec<ViewerBytes>) {
        out.extend(
            mem::take(&mut self.owed)
                .into_iter()
                .map(|(viewer, chunks)| ViewerBytes { viewer, chunks }),
        );
    }
}
