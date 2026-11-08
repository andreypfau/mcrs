/// The effect of a block on an entity inside it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InsideEffect {
    #[default]
    None,
    Cobweb,
    SweetBerryBush,
    PowderSnow,
    Honey,
}
