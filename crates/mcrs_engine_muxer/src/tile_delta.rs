use crate::entity_event::SubjectEvent;
use crate::tile_key::TileKey;

/// The events of one tile in one tick, sorted by subject. The events of one subject keep the
/// order in which they happened.
#[derive(Clone, Debug, PartialEq)]
pub struct TileDelta<D> {
    pub tile: TileKey,
    pub events: Vec<SubjectEvent<D>>,
}

/// What a world publishes at the end of a tick: the tiles whose subjects changed, sorted by
/// tile, at most one delta for each tile.
#[derive(Clone, Debug, PartialEq)]
pub struct TileDeltas<D> {
    pub tick: u64,
    pub tiles: Vec<TileDelta<D>>,
}

impl<D> TileDeltas<D> {
    /// Sorts deltas that several sources of one tick appended: one delta for each tile, sorted
    /// by tile, and the events of a tile sorted by subject. Each subject comes from one source,
    /// so the events of one subject keep their order.
    pub fn coalesce(&mut self) {
        self.tiles.sort_by_key(|delta| delta.tile);
        self.tiles.dedup_by(|later, kept| {
            if later.tile != kept.tile {
                return false;
            }
            kept.events.append(&mut later.events);
            true
        });
        for delta in &mut self.tiles {
            delta.events.sort_by_key(|event| event.subject);
        }
    }
}

impl<D> Default for TileDeltas<D> {
    fn default() -> Self {
        Self {
            tick: 0,
            tiles: Vec::new(),
        }
    }
}
