use mcrs_engine_store::ChunkVersion;

use crate::column_store::Delivery;
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

pub(crate) struct Wanted {
    pub(crate) delivery: Delivery,
    pub(crate) content: Option<Content>,
    pub(crate) light: Ask,
    pub(crate) written: Option<ChunkVersion>,
    pub(crate) changed: Changed,
    pub(crate) ring_changed: Changed,
}

pub(crate) struct Content {
    pub(crate) stored: StoredColumn,
    pub(crate) source: Source,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Dimension,
    DataLayer(ChunkVersion),
    Generation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ask {
    NotAsked,
    Asked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Changed {
    No,
    Yes,
}

impl Wanted {
    pub(crate) fn new(delivery: Delivery, written: Option<ChunkVersion>) -> Self {
        Self {
            delivery,
            content: None,
            light: Ask::NotAsked,
            written,
            changed: Changed::No,
            ring_changed: Changed::No,
        }
    }
}
