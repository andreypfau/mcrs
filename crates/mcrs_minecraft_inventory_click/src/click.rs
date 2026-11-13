use mcrs_minecraft_player_slots::PlayerMode;

/// One `container_click` packet, as far as the kernel reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub kind: ClickKind,
    pub button: u8,
    pub clicked: Clicked,
}

/// The click kinds that the kernel turns into operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ClickKind {
    PickUp,
    QuickMove,
    /// The swap with a hotbar key or with the off-hand key.
    Swap,
    Clone,
    Throw,
    PickUpAll,
}

/// The slot index of a click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clicked {
    /// The entry of the menu with this menu index.
    Entry(u16),
    /// The slot index -999: a click outside the window.
    Outside,
    /// Each other negative slot index: a click in the window that names no entry.
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

/// What a click reads of the player that sends it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickPlayer {
    /// [`PlayerMode::Creative`] gives the infinite materials of the clone click.
    pub mode: PlayerMode,
    /// The selected hotbar slot, which the insert rule fills first.
    pub selected: u8,
}
