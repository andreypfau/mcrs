#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoverKind {
    Own,
    Player,
    Piston,
    Shulker,
    ShulkerBox,
}
