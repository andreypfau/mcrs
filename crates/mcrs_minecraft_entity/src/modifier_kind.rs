/// Whether a save keeps a modifier (`attributes.md`, ATR-INS-005). Both kinds count in the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierKind {
    /// A save keeps the modifier.
    Permanent,
    /// A save drops the modifier.
    Transient,
}
