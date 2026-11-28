#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InsideEffect {
    #[default]
    None,
    Cobweb,
    SweetBerryBush,
    PowderSnow,
    Honey,
}
