/// The entities that an entity pushes in its own tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PushTargets {
    #[default]
    None,
    Each,
    /// Each candidate that is not a player, as a parrot pushes.
    NotPlayers,
}
