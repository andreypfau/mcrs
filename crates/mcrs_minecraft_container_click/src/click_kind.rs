#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "wire",
    derive(mcrs_minecraft_wire::Encode, mcrs_minecraft_wire::Decode)
)]
pub enum ClickKind {
    PickUp,
    QuickMove,
    Swap,
    Clone,
    Throw,
    Drag,
    PickUpAll,
}
