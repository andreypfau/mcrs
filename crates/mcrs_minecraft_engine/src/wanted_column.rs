use mcrs_engine_store::ChunkVersion;

use crate::column_store::Delivery;
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

/// A column that a view waits on, or a column that views hold and whose light is lit again.
pub(crate) struct Wanted {
    pub(crate) delivery: Delivery,
    pub(crate) content: Option<Content>,
    pub(crate) light: Ask,
    /// The last version the world wrote when the content was fetched. A later write makes
    /// the content stale.
    pub(crate) written: Option<ChunkVersion>,
    /// A block of the column changed since its content was taken from the dimension.
    pub(crate) changed: Changed,
    /// A block within the reach of light changed after the light was asked for, so the answer
    /// may lack it.
    pub(crate) ring_changed: Changed,
}

pub(crate) struct Content {
    pub(crate) stored: StoredColumn,
    pub(crate) source: Source,
}

/// Where the content of a column came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    Dimension,
    DataLayer(ChunkVersion),
    Generation,
}

/// Whether the light stage owes a wanted column its light.
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
