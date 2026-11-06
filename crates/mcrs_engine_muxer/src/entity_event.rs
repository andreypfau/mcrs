use crate::entity_id::EntityId;
use crate::tile_key::TileKey;

/// A change of one subject in one tick, as its tile sees it. `D` is the data of the game.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum EntityEvent<D> {
    /// The subject comes into the world in this tile.
    Spawn(D),
    Move(D),
    Data(D),
    /// The subject comes from another tile. A viewer of this tile that does not see `from`
    /// gets its spawn bundle.
    Enter {
        from: TileKey,
    },
    /// The subject goes to another tile. A viewer of this tile that does not see `to` gets its
    /// removal.
    Leave {
        to: TileKey,
    },
    /// The subject goes out of the world.
    Despawn,
}

/// An event of one subject.
#[derive(Clone, Debug, PartialEq)]
pub struct SubjectEvent<D> {
    pub subject: EntityId,
    pub event: EntityEvent<D>,
}
