use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_item::keys::MenuType;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_protocol::item::{HashedStack, RawStack};
use mcrs_minecraft_registry::Id;

use crate::drag::Drag;
use crate::slot::Slot;

/// The main inventory and hotbar slots every container menu ends with.
pub const PLAYER_MENU_SLOTS: usize = (slots::HOTBAR.end - slots::MAIN.start) as usize;

#[derive(Component, Debug)]
pub struct Menu {
    pub container_id: u8,
    pub state_id: u16,
    pub drag: Option<Drag>,
}

impl Menu {
    pub fn next_state_id(&mut self) -> u16 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }
}

/// Menu index → slot.
#[derive(Component, Debug)]
pub struct MenuLayout(pub Vec<Slot>);

#[derive(Component, Debug)]
#[relationship(relationship_target = MenusOf)]
pub struct MenuViewer(pub Entity);

#[derive(Component, Debug)]
#[relationship_target(relationship = MenuViewer, linked_spawn)]
pub struct MenusOf(Vec<Entity>);

#[derive(Component, Debug)]
pub struct CurrentMenu(pub Entity);

/// The block entity a container menu shows; the menu closes with it.
#[derive(Component, Debug)]
pub struct MenuContainer(pub Entity);

/// The menu's own slots take only stacks that fit inside container items.
#[derive(Component, Debug)]
pub struct ShulkerBoxSlots;

/// What the viewer's client holds for one slot, as far as the server knows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Remote {
    #[default]
    Unknown,
    /// The client acknowledged this in a click; it stands until the slot is
    /// compared against the server's stack.
    Claimed(Option<HashedStack>),
    Known(RawStack),
}

/// The viewer's copy of the menu; the slot sync sends only what differs.
#[derive(Component, Debug, Default)]
pub struct RemoteSlots {
    pub slots: Vec<Remote>,
    pub carried: Remote,
    /// The whole menu goes out as one content packet.
    pub full: bool,
}

impl RemoteSlots {
    pub fn claim(&mut self, index: usize, hashed: Option<HashedStack>) {
        if self.slots.len() <= index {
            self.slots.resize(index + 1, Remote::Unknown);
        }
        self.slots[index] = Remote::Claimed(hashed);
    }
}

pub fn player_menu_layout(player: Entity) -> Vec<Slot> {
    (0..slots::MENU_COUNT as u16)
        .map(|index| Slot::new(player, index))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuSlots {
    pub own: u16,
    pub player_slots: bool,
    pub trailing_result: bool,
}

const fn own(own: u16) -> MenuSlots {
    MenuSlots {
        own,
        player_slots: true,
        trailing_result: false,
    }
}

/// Vanilla menus add their own slots first, then the player's main and hotbar
/// rows; the lectern adds none of the player's and the crafter appends a
/// non-interactive result slot after them.
pub fn menu_slots(menu: Id<MenuType>) -> Option<MenuSlots> {
    use MenuType::*;
    Some(match MenuType::from_id(menu)? {
        Generic9x1 | Generic3x3 => own(9),
        Generic9x2 => own(18),
        Generic9x3 | ShulkerBox => own(27),
        Generic9x4 => own(36),
        Generic9x5 => own(45),
        Generic9x6 => own(54),
        Crafter3x3 => MenuSlots {
            own: 9,
            player_slots: true,
            trailing_result: true,
        },
        Anvil | BlastFurnace | Furnace | Grindstone | Merchant | Smoker | CartographyTable => {
            own(3)
        }
        Beacon => own(1),
        BrewingStand | Hopper => own(5),
        Crafting => own(10),
        Enchantment | Stonecutter => own(2),
        Lectern => MenuSlots {
            own: 1,
            player_slots: false,
            trailing_result: false,
        },
        Loom | Smithing => own(4),
    })
}

/// The container's own slots, then the player's main and hotbar rows.
pub fn container_menu_layout(container: Entity, player: Entity, menu: MenuSlots) -> Vec<Slot> {
    let mut layout: Vec<Slot> = (0..menu.own)
        .map(|index| Slot::new(container, index))
        .collect();
    if menu.player_slots {
        layout.extend(
            slots::MAIN
                .chain(slots::HOTBAR)
                .map(|index| Slot::new(player, index)),
        );
    }
    if menu.trailing_result {
        layout.push(Slot::new(container, menu.own));
    }
    layout
}
