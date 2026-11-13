/// What a caret move does with the other end of the selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// The anchor moves with the caret, so nothing stays selected.
    #[default]
    Move,
    /// The anchor stays, so the selection grows or shrinks.
    Keep,
}
