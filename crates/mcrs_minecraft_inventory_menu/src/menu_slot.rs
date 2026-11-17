use mcrs_minecraft_inventory_player::Holder;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuSlot {
    pub holder: Holder,
    pub index: u16,
}

impl MenuSlot {
    #[must_use]
    pub const fn new(holder: Holder, index: u16) -> Self {
        Self { holder, index }
    }
}
