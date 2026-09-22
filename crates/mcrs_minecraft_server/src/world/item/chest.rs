use crate::world::bus::PacketPayload;
use crate::world::entity::item::EYE_HEIGHT;
use crate::world::inventory::NextContainerId;
use crate::world::item::click::return_carried;
use crate::world::item::sync::to;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, Messages};
use bevy_ecs::relationship::RelationshipTarget;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_inventory::{
    CurrentMenu, Menu, MenuContainer, MenuLayout, MenuSlots, MenuViewer, MenusOf, RemoteSlots,
    container_menu_layout,
};
use mcrs_minecraft_item::SlotTable;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::world::storage::block_entity::BlockEntityPos;
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

fn inventory_menu(world: &World, player: Entity) -> Option<Entity> {
    world.get::<MenusOf>(player)?.iter().find(|&menu| {
        world
            .get::<Menu>(menu)
            .is_some_and(|menu| menu.container_id == 0)
    })
}

/// Returns the player to their inventory menu; the client is told when it
/// did not ask for the close itself.
pub fn close_container_menu(world: &mut World, player: Entity, menu: Entity, notify: bool) {
    let Some(container_id) = world.get::<Menu>(menu).map(|menu| menu.container_id) else {
        return;
    };
    return_carried(world, player);
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
            tracing::debug!(container = ?req.container, slots = table.len(), "a container without a chest menu");
            continue;
        }
        if world.get::<MenuContainer>(current).is_some() {
            close_container_menu(world, req.player, current, true);
        }
        let mut player = world.entity_mut(req.player);
        let mut next = player.entry::<NextContainerId>().or_default();
        next.get_mut().0 = next.get().0 % 100 + 1;
        let container_id = next.get().0;
        let layout = container_menu_layout(
            req.container,
            req.player,
            MenuSlots {
                own: (CHEST_ROWS * 9) as u16,
                player_slots: true,
                trailing_result: false,
            },
        );
        let menu = world
            .spawn((
                Menu {
                    container_id,
                    state_id: 0,
                },
                MenuLayout(layout),
                MenuViewer(req.player),
                MenuContainer(req.container),
                RemoteSlots {
                    full: true,
                    ..Default::default()
                },
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
    }
}

/// ponytail: the block interaction range attribute is not modelled, so the
/// vanilla default stands in. Upgrade: read the player's attribute.
const BLOCK_INTERACTION_RANGE: f64 = 4.5;
const STILL_VALID_BUFFER: f64 = 4.0;

fn within_reach(world: &World, player: Entity, container: Entity) -> bool {
    let Some((transform, pos)) = world
        .get::<Transform>(player)
        .zip(world.get::<BlockEntityPos>(container))
    else {
        return false;
    };
    let eye = transform.translation + DVec3::new(0.0, EYE_HEIGHT, 0.0);
    let min = pos.0.as_ivec3().as_dvec3();
    let gap = (min - eye).max(eye - (min + DVec3::ONE)).max(DVec3::ZERO);
    let range = BLOCK_INTERACTION_RANGE + STILL_VALID_BUFFER;
    gap.length_squared() < range * range
}

/// A container menu whose block entity left with its section, or whose
/// viewer walked out of reach, closes on the client too.
pub fn close_dead_menus(world: &mut World) {
    let dead: Vec<(Entity, Entity)> = world
        .query::<(Entity, &MenuContainer, &MenuViewer)>()
        .iter(world)
        .filter(|(_, container, viewer)| !within_reach(world, viewer.0, container.0))
        .map(|(menu, _, viewer)| (menu, viewer.0))
        .collect();
    for (menu, player) in dead {
        close_container_menu(world, player, menu, true);
    }
}
