use bevy::prelude::Entity;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_menu::MenuLayout;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuStacks {
    entries: Vec<Option<Entity>>,
    carried: Option<Entity>,
}

impl MenuStacks {
    pub fn collect(
        &mut self,
        layout: &MenuLayout,
        parts: &MenuParts,
        cursor: Entity,
        occupant: impl Fn(SlotRef) -> Option<Entity>,
    ) {
        self.entries.clear();
        self.entries.extend((0..layout.len()).map(|menu| {
            layout
                .entry(menu)
                .and_then(|entry| parts.slot(entry))
                .and_then(&occupant)
        }));
        self.carried = occupant(SlotRef {
            holder: cursor,
            index: 0,
        });
    }

    #[must_use]
    pub fn get(&self, menu: u16) -> Option<Entity> {
        self.entries.get(usize::from(menu)).copied().flatten()
    }

    #[must_use]
    pub fn carried(&self) -> Option<Entity> {
        self.carried
    }
}
