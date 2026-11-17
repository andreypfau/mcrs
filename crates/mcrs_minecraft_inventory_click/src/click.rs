use mcrs_minecraft_inventory_player::PlayerMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub kind: ClickKind,
    pub button: u8,
    pub clicked: Clicked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClickKind {
    PickUp,
    QuickMove,
    Swap,
    Clone,
    Throw,
    Drag,
    PickUpAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clicked {
    Entry(u16),
    Outside,
    Nowhere,
}

impl From<i16> for Clicked {
    fn from(index: i16) -> Self {
        match u16::try_from(index) {
            Ok(entry) => Self::Entry(entry),
            Err(_) if index == -999 => Self::Outside,
            Err(_) => Self::Nowhere,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickPlayer {
    pub mode: PlayerMode,
    pub selected: u8,
}
