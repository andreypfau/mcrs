use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_keys as keys;
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

const MENU_SLOTS: [Option<MenuSlots>; keys::menu::NAMES.len()] = {
    let mut table = [None; keys::menu::NAMES.len()];
    table[keys::menu::GENERIC_9X1.index()] = Some(own(9));
    table[keys::menu::GENERIC_9X2.index()] = Some(own(18));
    table[keys::menu::GENERIC_9X3.index()] = Some(own(27));
    table[keys::menu::GENERIC_9X4.index()] = Some(own(36));
    table[keys::menu::GENERIC_9X5.index()] = Some(own(45));
    table[keys::menu::GENERIC_9X6.index()] = Some(own(54));
    table[keys::menu::GENERIC_3X3.index()] = Some(own(9));
    table[keys::menu::CRAFTER_3X3.index()] = Some(MenuSlots {
        own: 9,
        player_slots: true,
        trailing_result: true,
    });
    table[keys::menu::ANVIL.index()] = Some(own(3));
    table[keys::menu::BEACON.index()] = Some(own(1));
    table[keys::menu::BLAST_FURNACE.index()] = Some(own(3));
    table[keys::menu::BREWING_STAND.index()] = Some(own(5));
    table[keys::menu::CRAFTING.index()] = Some(own(10));
    table[keys::menu::ENCHANTMENT.index()] = Some(own(2));
    table[keys::menu::FURNACE.index()] = Some(own(3));
    table[keys::menu::GRINDSTONE.index()] = Some(own(3));
    table[keys::menu::HOPPER.index()] = Some(own(5));
    table[keys::menu::LECTERN.index()] = Some(MenuSlots {
        own: 1,
        player_slots: false,
        trailing_result: false,
    });
    table[keys::menu::LOOM.index()] = Some(own(4));
    table[keys::menu::MERCHANT.index()] = Some(own(3));
    table[keys::menu::SHULKER_BOX.index()] = Some(own(27));
    table[keys::menu::SMITHING.index()] = Some(own(4));
    table[keys::menu::SMOKER.index()] = Some(own(3));
    table[keys::menu::CARTOGRAPHY_TABLE.index()] = Some(own(3));
    table[keys::menu::STONECUTTER.index()] = Some(own(2));
    table
};

/// Vanilla menus add their own slots first, then the player's main and hotbar
/// rows; the lectern adds none of the player's and the crafter appends a
/// non-interactive result slot after them.
pub fn menu_slots(menu: Id<keys::Menu>) -> Option<MenuSlots> {
    MENU_SLOTS.get(menu.index()).copied().flatten()
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
