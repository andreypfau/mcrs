use bevy::prelude::Entity;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item::slots;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStacks {
    stacks: [Option<Entity>; slots::COUNT],
}

impl PlayerStacks {
    #[must_use]
    pub fn collect(
        layout: &MenuLayout,
        parts: &MenuParts,
        cursor: Entity,
        occupant: impl Fn(SlotRef) -> Option<Entity>,
    ) -> Self {
        let mut stacks = [None; slots::COUNT];
        for (menu, stack) in (0u16..).zip(stacks.iter_mut()) {
            let slot = if menu == slots::CARRIED {
                Some(SlotRef {
                    holder: cursor,
                    index: 0,
                })
            } else {
                layout.entry(menu).and_then(|entry| parts.slot(entry))
            };
            *stack = slot.and_then(&occupant);
        }
        Self { stacks }
    }

    #[must_use]
    pub fn get(&self, menu: u16) -> Option<Entity> {
        self.stacks.get(usize::from(menu)).copied().flatten()
    }
}
