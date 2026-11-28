use crate::ArmourSlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    Armour(ArmourSlot),
    OffHand,
}
