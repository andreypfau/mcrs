use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_menu::MenuEntry;

pub const MAX_PARTS: usize = 4;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuParts {
    holders: [Entity; MAX_PARTS],
    len: usize,
}

impl MenuParts {
    #[must_use]
    pub fn new(holders: impl IntoIterator<Item = Entity>) -> Option<Self> {
        let mut parts = Self {
            holders: [Entity::PLACEHOLDER; MAX_PARTS],
            len: 0,
        };
        for holder in holders {
            *parts.holders.get_mut(parts.len)? = holder;
            parts.len += 1;
        }
        Some(parts)
    }

    #[must_use]
    pub fn holder(&self, part: u8) -> Option<Entity> {
        self.holders().get(usize::from(part)).copied()
    }

    #[must_use]
    pub fn slot(&self, entry: MenuEntry) -> Option<SlotRef> {
        let holder = self.holder(entry.part)?;
        Some(SlotRef {
            holder,
            index: entry.index,
        })
    }

    pub(crate) fn holders(&self) -> &[Entity] {
        self.holders.get(..self.len).unwrap_or_default()
    }
}
