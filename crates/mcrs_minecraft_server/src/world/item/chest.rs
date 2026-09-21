use crate::world::bus::PacketPayload;
use crate::world::item::click::{PLAYER_MENU_CELLS, insert_or_drop};
use crate::world::item::menu::{CurrentMenu, Menu, MenuLayout, MenuViewer, MenusOf};
use crate::world::item::sync::{MenuResync, to};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, Messages};
use bevy_ecs::world::World;
use mcrs_minecraft_item::{Items, SlotTable, slots};
use mcrs_minecraft_protocol::Text;

/// ponytail: no menu registry is loaded, so the generic 9x3 id is the
/// vanilla constant. Upgrade: a `minecraft:menu` snapshot in RegistryAccess.
const GENERIC_9X3: i32 = 2;
const CHEST_ROWS: usize = 3;

#[derive(Message, Debug)]
pub struct OpenContainerRequest {
    pub player: Entity,
    pub container: Entity,
}

/// The block entity a container menu shows; the menu closes with it.
#[derive(Component, Debug)]
pub struct MenuContainer(pub Entity);

fn inventory_menu(world: &World, player: Entity) -> Option<Entity> {
    world
        .get::<MenusOf>(player)?
        .entities()
        .iter()
        .copied()
        .find(|&menu| {
            world
                .get::<Menu>(menu)
                .is_some_and(|menu| menu.container_id == 0)
        })
}

/// Returns the player to their inventory menu; the client is told when it
/// did not ask for the close itself.
pub fn close_container_menu(world: &mut World, player: Entity, menu: Entity, notify: bool) {
    let items = world.resource::<Items>().clone();
    let Some(container_id) = world.get::<Menu>(menu).map(|menu| menu.container_id) else {
        return;
    };
    if let Some(carried) = world
        .get::<SlotTable>(player)
        .and_then(|table| table.get(slots::CARRIED))
        && let Err(error) = insert_or_drop(world, player, carried, &items)
    {
        tracing::debug!(%error, ?player, "the carried stack could not be returned on close");
    }
    world.despawn(menu);
    match inventory_menu(world, player) {
        Some(inventory) => {
            world.entity_mut(player).insert(CurrentMenu(inventory));
        }
        None => {
            world.entity_mut(player).remove::<CurrentMenu>();
        }
    }
    if notify {
        let packet = to(world, player, PacketPayload::ContainerClose(container_id));
        world
            .resource_mut::<Messages<crate::world::bus::OutboundPlayerPacket>>()
            .write(packet);
    }
}

pub fn open_containers(world: &mut World) {
    let requests: Vec<OpenContainerRequest> = world
        .resource_mut::<Messages<OpenContainerRequest>>()
        .drain()
        .collect();
    for req in requests {
        let Some(current) = world
            .get::<CurrentMenu>(req.player)
            .map(|current| current.0)
        else {
            continue;
        };
        let Some(table) = world.get::<SlotTable>(req.container) else {
            continue;
        };
        if table.len() != CHEST_ROWS * 9 {
            tracing::debug!(container = ?req.container, cells = table.len(), "a container without a chest menu");
            continue;
        }
        let Some(previous_id) = world.get::<Menu>(current).map(|menu| menu.container_id) else {
            continue;
        };
        if world.get::<MenuContainer>(current).is_some() {
            close_container_menu(world, req.player, current, true);
        }
        let container_id = previous_id % 100 + 1;
        let layout: Vec<(Entity, u16)> = (0..(CHEST_ROWS * 9) as u16)
            .map(|index| (req.container, index))
            .chain((slots::MAIN.start..slots::HOTBAR.end).map(|index| (req.player, index)))
            .collect();
        debug_assert_eq!(layout.len(), CHEST_ROWS * 9 + PLAYER_MENU_CELLS);
        let menu = world
            .spawn((
                Menu {
                    container_id,
                    state_id: 0,
                },
                MenuLayout(layout),
                MenuViewer(req.player),
                MenuContainer(req.container),
            ))
            .id();
        world.entity_mut(req.player).insert(CurrentMenu(menu));
        let packet = to(
            world,
            req.player,
            PacketPayload::OpenScreen {
                container_id,
                menu_type: GENERIC_9X3,
                title: Text::translate("container.chest", Vec::new()),
            },
        );
        world
            .resource_mut::<Messages<crate::world::bus::OutboundPlayerPacket>>()
            .write(packet);
        world.resource_mut::<MenuResync>().menus_full.push(menu);
    }
}

/// A container menu whose block entity left with its section closes on the
/// client too.
pub fn close_dead_menus(world: &mut World) {
    let dead: Vec<(Entity, Entity)> = world
        .query::<(Entity, &MenuContainer, &MenuViewer)>()
        .iter(world)
        .filter(|(_, container, _)| world.get_entity(container.0).is_err())
        .map(|(menu, _, viewer)| (menu, viewer.0))
        .collect();
    for (menu, player) in dead {
        close_container_menu(world, player, menu, true);
    }
}
