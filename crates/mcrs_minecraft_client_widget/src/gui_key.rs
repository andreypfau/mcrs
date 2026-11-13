/// A key that the widgets of a screen use (WID-FOC-003). The screen handles Escape before its
/// widgets.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuiKey {
    Tab,
    Enter,
    Space,
    NumpadEnter,
    Up,
    Down,
    Left,
    Right,
    Backspace,
    Delete,
    Home,
    End,
    /// A digit key of the top row or of the keypad, from 0 to 9.
    Digit(u8),
}

impl GuiKey {
    pub(crate) const fn is_activation(self) -> bool {
        matches!(self, Self::Enter | Self::Space | Self::NumpadEnter)
    }

    pub(crate) const fn is_arrow(self) -> bool {
        matches!(self, Self::Up | Self::Down | Self::Left | Self::Right)
    }
}
