use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use mcrs_minecraft_entity_key::TileKey;

use crate::EffectBuffer;
use crate::Keyed;

/// Per tile, the entries that its task applies: late AI decisions, or the writes or the claims
/// that the tasks binned for it.
#[derive(Clone, Debug, PartialEq)]
pub struct TileQueues<T> {
    tiles: BTreeMap<TileKey, Vec<T>>,
}

impl<T> Default for TileQueues<T> {
    fn default() -> Self {
        Self {
            tiles: BTreeMap::new(),
        }
    }
}

impl<T> TileQueues<T> {
    /// Takes the queue of `tile`; empty when nothing targets it.
    pub fn take(&mut self, tile: TileKey) -> Vec<T> {
        self.tiles.remove(&tile).unwrap_or_default()
    }
}

impl<E> TileQueues<Keyed<E>> {
    /// Moves the bins of the buffer of one task to the queues of their tiles.
    pub fn gather(&mut self, buffer: &mut EffectBuffer<E>) {
        for bin in buffer.drain() {
            match self.tiles.entry(bin.tile) {
                Entry::Vacant(vacant) => {
                    vacant.insert(bin.entries);
                }
                Entry::Occupied(mut occupied) => occupied.get_mut().extend(bin.entries),
            }
        }
    }

    /// Takes the queue of `tile` in the order of application: by target key, then writer key,
    /// then sequence. The order does not depend on the tasks that made the entries.
    pub fn take_in_order(&mut self, tile: TileKey) -> Vec<Keyed<E>> {
        let mut entries = self.take(tile);
        entries.sort_unstable_by_key(|entry| (entry.target, entry.writer, entry.sequence));
        entries
    }
}
