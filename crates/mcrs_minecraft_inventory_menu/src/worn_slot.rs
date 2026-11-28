use mcrs_minecraft_inventory_player::ArmourSlot;
use mcrs_minecraft_inventory_player::EquipSlot;
use mcrs_minecraft_item_stack::EquippableSlot;

#[must_use]
pub const fn worn_slot(slot: EquippableSlot) -> Option<EquipSlot> {
    match slot {
        EquippableSlot::Feet => Some(EquipSlot::Armour(ArmourSlot::Feet)),
        EquippableSlot::Legs => Some(EquipSlot::Armour(ArmourSlot::Legs)),
        EquippableSlot::Chest => Some(EquipSlot::Armour(ArmourSlot::Chest)),
        EquippableSlot::Head => Some(EquipSlot::Armour(ArmourSlot::Head)),
        EquippableSlot::Offhand => Some(EquipSlot::OffHand),
        _ => None,
    }
}
