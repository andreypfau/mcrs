use alloc::vec::Vec;

use crate::node_hash::MapEntry;

#[derive(Clone, Debug, Default)]
pub struct HashScratch {
    pub(crate) entries: Vec<MapEntry>,
}
