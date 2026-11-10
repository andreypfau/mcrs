/// The identity of a viewer, chosen by the game. It is unique among the viewers of a muxer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewerId(pub u64);

/// The identity of a world, chosen by the game. [`Muxers`](crate::Muxers) keeps one
/// [`Muxer`](crate::Muxer) for each world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorldId(pub u64);
