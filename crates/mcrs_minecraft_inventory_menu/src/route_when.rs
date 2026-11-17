use mcrs_minecraft_inventory_player::EquipSlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RouteWhen {
    Always,
    Wears { slot: EquipSlot, target: u16 },
}
