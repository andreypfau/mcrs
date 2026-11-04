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
