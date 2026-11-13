/// How far a caret move or a removal goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    /// One character.
    #[default]
    Char,
    /// To the next word boundary.
    Word,
}
