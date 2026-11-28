use mcrs_minecraft_inventory_player::EquipSlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RouteWhen {
    Always,
    Wears { slot: EquipSlot, target: u16 },
}

impl RouteWhen {
    #[must_use]
    pub fn holds(self, worn: Option<EquipSlot>, holds_empty: impl FnOnce(u16) -> bool) -> bool {
        match self {
            Self::Always => true,
            Self::Wears { slot, target } => worn == Some(slot) && holds_empty(target),
        }
    }
}
