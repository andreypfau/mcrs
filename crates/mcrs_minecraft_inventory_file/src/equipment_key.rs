use mcrs_minecraft_inventory_player::ArmourSlot;
use mcrs_minecraft_inventory_player::OFF_HAND;

pub(crate) const INVENTORY_KEY: &str = "Inventory";
pub(crate) const SELECTED_KEY: &str = "SelectedItemSlot";
pub(crate) const EQUIPMENT_KEY: &str = "equipment";
pub(crate) const FILE_KEYS: [&str; 3] = [INVENTORY_KEY, SELECTED_KEY, EQUIPMENT_KEY];

pub(crate) const EQUIPMENT: [EquipmentKey; 5] = [
    EquipmentKey {
        key: "offhand",
        index: OFF_HAND,
    },
    EquipmentKey {
        key: "feet",
        index: ArmourSlot::Feet.index(),
    },
    EquipmentKey {
        key: "legs",
        index: ArmourSlot::Legs.index(),
    },
    EquipmentKey {
        key: "chest",
        index: ArmourSlot::Chest.index(),
    },
    EquipmentKey {
        key: "head",
        index: ArmourSlot::Head.index(),
    },
];

pub(crate) struct EquipmentKey {
    pub(crate) key: &'static str,
    pub(crate) index: u16,
}

pub(crate) fn is_slot_key(key: &str) -> bool {
    EQUIPMENT.iter().any(|equipment| equipment.key == key)
}
