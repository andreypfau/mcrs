use crate::EquipSlot;

/// What a place filter reads of the stack that goes into a slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    /// The slot that the `minecraft:equippable` component of the stack names, when that
    /// component has no `allowed_entities` or they contain `minecraft:player`.
    pub wear: Option<EquipSlot>,
}
