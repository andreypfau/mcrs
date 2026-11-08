/// The source of one call of the move procedure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MoverKind {
    /// The tick of the entity itself.
    Own,
    /// A displacement that the client of a player sends.
    Player,
    Piston,
    Shulker,
    ShulkerBox,
}
