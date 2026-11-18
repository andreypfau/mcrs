use bevy::prelude::{Entity, Query};
use mcrs_minecraft_engine_inventory_menu::{MenuParts, Menus};
use mcrs_minecraft_engine_item::{SlotRef, Slots};
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_player_holders::PlayerHolders;

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
    pub(crate) fn of_player(
        player: &PlayerHolders,
        player_menus: &Menus,
        menus: &Query<(&MenuLayout, &MenuParts)>,
        holders: &Query<&Slots>,
    ) -> Option<Self> {
        let (layout, parts) = menus.get(player_menus.iter().next()?).ok()?;
        Some(Self::collect(layout, parts, player.cursor, |slot| {
            holders.get(slot.holder).ok()?.get(usize::from(slot.index))
        }))
    }

    #[must_use]
    pub fn get(&self, menu: u16) -> Option<Entity> {
        self.stacks.get(usize::from(menu)).copied().flatten()
    }
}
