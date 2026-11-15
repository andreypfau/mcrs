use crate::EquipSlot;

/// What a place filter reads of the stack that goes into a slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offer {
    /// The slot that the `minecraft:equippable` component of the stack names, when that
    /// component has no `allowed_entities` or they contain `minecraft:player`.
    pub wear: Option<EquipSlot>,
    /// Whether the item of the stack is in the item tag `mcrs:offhand_allowed`.
    pub off_hand_tag: OffHandTag,
}

/// The membership of an item in the item tag `mcrs:offhand_allowed`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OffHandTag {
    Member,
    #[default]
    NotMember,
}
