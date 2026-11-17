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
