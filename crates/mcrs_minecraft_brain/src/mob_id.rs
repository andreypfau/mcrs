/// The handle of a mob in the host, opaque to the stage. A decision names its mob by it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MobId(pub u64);

/// The handle of a player in the host, opaque to the stage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(pub u64);
