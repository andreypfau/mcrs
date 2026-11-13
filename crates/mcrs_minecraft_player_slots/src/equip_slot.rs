use crate::ArmourSlot;

/// A slot of the player inventory that the `slot` of the `minecraft:equippable` component of a
/// stack can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    Armour(ArmourSlot),
    OffHand,
}
