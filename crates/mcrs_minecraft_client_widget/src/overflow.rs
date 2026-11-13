/// What a one-line text widget does with a text wider than its limit (WID-TXT-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Overflow {
    /// The longest start of the text that fits, then `...`.
    #[default]
    Cut,
    /// The label scroll of WID-BTN-005.
    Scroll,
}
