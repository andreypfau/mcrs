use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_protocol::item::{HashedStack, RawStack};

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

/// Vanilla menus add their own slots first, then the player's main and hotbar
/// rows; the lectern adds none of the player's and the crafter appends a
/// non-interactive result slot after them.
pub fn menu_slots(menu_type: &str) -> Option<MenuSlots> {
    let own = |own| MenuSlots {
        own,
        player_slots: true,
        trailing_result: false,
    };
    Some(match menu_type {
        "minecraft:generic_9x1" => own(9),
        "minecraft:generic_9x2" => own(18),
        "minecraft:generic_9x3" => own(27),
        "minecraft:generic_9x4" => own(36),
        "minecraft:generic_9x5" => own(45),
        "minecraft:generic_9x6" => own(54),
        "minecraft:generic_3x3" => own(9),
        "minecraft:crafter_3x3" => MenuSlots {
            own: 9,
            player_slots: true,
            trailing_result: true,
        },
        "minecraft:anvil" => own(3),
        "minecraft:beacon" => own(1),
        "minecraft:blast_furnace" => own(3),
        "minecraft:brewing_stand" => own(5),
        "minecraft:crafting" => own(10),
        "minecraft:enchantment" => own(2),
        "minecraft:furnace" => own(3),
        "minecraft:grindstone" => own(3),
        "minecraft:hopper" => own(5),
        "minecraft:lectern" => MenuSlots {
            own: 1,
            player_slots: false,
            trailing_result: false,
        },
        "minecraft:loom" => own(4),
        "minecraft:merchant" => own(3),
        "minecraft:shulker_box" => own(27),
        "minecraft:smithing" => own(4),
        "minecraft:smoker" => own(3),
        "minecraft:cartography_table" => own(3),
        "minecraft:stonecutter" => own(2),
        _ => return None,
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
