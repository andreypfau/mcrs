use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use bytes::Bytes;
use mcrs_engine_muxer::{ViewerBytes, ViewerId};

/// The records that a codec owes each viewer at the end of the tick.
#[derive(Debug, Default)]
pub(crate) struct Delivery {
    owed: BTreeMap<ViewerId, Vec<Bytes>>,
    /// The viewers that left the muxer while their own subject is still in this world. They
    /// get no more bytes from it.
    forgotten: BTreeSet<ViewerId>,
}

impl Delivery {
    pub(crate) fn owe(&mut self, viewer: ViewerId, packet: Bytes) {
        if !self.forgotten.contains(&viewer) {
            self.owed.entry(viewer).or_default().push(packet);
        }
    }

    /// Drops what `viewer` is owed.
    pub(crate) fn drop_owed(&mut self, viewer: ViewerId) {
        self.owed.remove(&viewer);
    }

    /// Owes `viewer` nothing more until [`Delivery::remember`].
    pub(crate) fn forget(&mut self, viewer: ViewerId) {
        self.forgotten.insert(viewer);
    }

    pub(crate) fn remember(&mut self, viewer: ViewerId) {
        self.forgotten.remove(&viewer);
    }

    /// Appends what each viewer is owed, and owes nothing after.
    pub(crate) fn drain(&mut self, out: &mut Vec<ViewerBytes>) {
        out.extend(
            mem::take(&mut self.owed)
                .into_iter()
                .map(|(viewer, chunks)| ViewerBytes { viewer, chunks }),
        );
    }
}
