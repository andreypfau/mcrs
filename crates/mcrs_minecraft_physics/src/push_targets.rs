#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PushTargets {
    #[default]
    None,
    Each,
    NotPlayers,
}
