/// The sprite that fills the bounds of a button or an icon button (WID-BTN-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSprite {
    Normal,
    Highlighted,
    Disabled,
}

impl ButtonSprite {
    pub(crate) const fn of(active: bool, highlighted: bool) -> Self {
        match (active, highlighted) {
            (true, false) => Self::Normal,
            (true, true) => Self::Highlighted,
            (false, _) => Self::Disabled,
        }
    }
}
