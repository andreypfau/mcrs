use crate::node_hash::MapEntry;
use crate::node_hash::NodeHash;

const CAPACITY: usize = 11;

pub(crate) struct FieldMap {
    entries: [MapEntry; CAPACITY],
    len: usize,
}

impl FieldMap {
    pub(crate) fn new() -> Self {
        let empty = MapEntry {
            key: NodeHash::absent(),
            value: NodeHash::absent(),
        };
        Self {
            entries: [empty; CAPACITY],
            len: 0,
        }
    }

    pub(crate) fn put(&mut self, field: &str, value: NodeHash) {
        if let Some(entry) = self.entries.get_mut(self.len) {
            *entry = MapEntry {
                key: NodeHash::string(field),
                value,
            };
            self.len += 1;
        }
    }

    pub(crate) fn finish(mut self) -> NodeHash {
        NodeHash::map(self.entries.get_mut(..self.len).unwrap_or_default())
    }
}
