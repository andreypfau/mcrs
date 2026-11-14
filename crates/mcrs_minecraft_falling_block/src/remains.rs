use crate::fall::ItemDrop;

/// The value of the game rule `minecraft:entity_drops`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityDrops {
    On,
    Off,
}

/// What a falling block leaves when it ends as its item (FBK-LND-006, FBK-TCK-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Remains {
    /// One item entity of the carried block, when the block has an item.
    Item,
    Nothing,
}

impl Remains {
    /// An item when the drop flag and the game rule both allow it.
    #[must_use]
    pub fn of(item_drop: ItemDrop, entity_drops: EntityDrops) -> Self {
        match (item_drop, entity_drops) {
            (ItemDrop::Allowed, EntityDrops::On) => Self::Item,
            (ItemDrop::Cancelled, _) | (_, EntityDrops::Off) => Self::Nothing,
        }
    }
}
