use alloc::vec::Vec;

use crate::node_hash::MapEntry;

/// The buffer that the hash of a component sorts the entries of a map in. Keep one and lend it to
/// each call: after the largest map, a hash makes no allocation.
#[derive(Clone, Debug, Default)]
pub struct HashScratch {
    pub(crate) entries: Vec<MapEntry>,
}
