use crate::world::bus::PacketPayload;
use crate::world::item::sync::to;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{With, Without};
use bevy_ecs::world::World;
use mcrs_minecraft_inventory::{
    CurrentMenu, Menu, MenuLayout, MenuViewer, RemoteSlots, player_menu_layout,
};
use mcrs_minecraft_item::{SelectedHotbarSlot, SlotTable};
use mcrs_minecraft_level::entity::player::Player;

pub fn open_menus(world: &mut World) {
    let joined: Vec<Entity> = world
        .query_filtered::<Entity, (With<Player>, With<SlotTable>, Without<CurrentMenu>)>()
        .iter(world)
        .collect();
    for player in joined {
        let menu = world
            .spawn((
                Menu {
                    container_id: 0,
                    state_id: 0,
                },
                MenuLayout(player_menu_layout(player)),
                MenuViewer(player),
                RemoteSlots::fresh(),
            ))
            .id();
        world.entity_mut(player).insert(CurrentMenu(menu));
        let selected = world
            .get::<SelectedHotbarSlot>(player)
            .map_or(0, |selected| selected.0);
        let packet = to(world, player, PacketPayload::SetHeldSlot(selected));
        world
            .resource_mut::<Messages<crate::world::bus::OutboundPlayerPacket>>()
            .write(packet);
    }
}
