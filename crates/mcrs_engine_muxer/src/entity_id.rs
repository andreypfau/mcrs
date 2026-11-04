/// The identity of an entity in the whole world. It stays the same when the entity moves to
/// another tile or to another cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub u64);

/// The identity of a viewer, chosen by the game. It is unique among the viewers of a muxer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewerId(pub u64);
