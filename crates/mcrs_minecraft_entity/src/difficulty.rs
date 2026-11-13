/// The difficulty of a world.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}
