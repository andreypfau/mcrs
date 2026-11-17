use crate::fall::ItemDrop;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntityDrops {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Remains {
    Item,
    Nothing,
}

impl Remains {
    #[must_use]
    pub fn of(item_drop: ItemDrop, entity_drops: EntityDrops) -> Self {
        match (item_drop, entity_drops) {
            (ItemDrop::Allowed, EntityDrops::On) => Self::Item,
            (ItemDrop::Cancelled, _) | (_, EntityDrops::Off) => Self::Nothing,
        }
    }
}
