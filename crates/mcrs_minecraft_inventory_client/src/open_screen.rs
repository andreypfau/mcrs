use bevy_ecs::entity::Entity;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Single;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuCommandsExt;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol_game::clientbound::ClientboundOpenScreen;

use crate::RemoteMenu;
use crate::clear_first_slot::clear_first_slot;

pub(crate) fn open_screen(
    received: On<Received<ClientboundOpenScreen<'static>>>,
    player: Single<(Entity, &PlayerHolders)>,
    holders: Query<&Slots>,
    mut commands: Commands,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    let Some(layout) = MenuLayout::of(packet.menu_type) else {
        return;
    };
    let title = packet.title.to_text().unwrap_or_else(|error| {
        tracing::warn!(%error, "the menu title from the server is not a text");
        MenuTitle::default().0
    });
    let (viewer, player_holders) = *player;
    let holder = commands
        .spawn(Slots::new(usize::from(
            layout.part_len(MenuLayout::GENERIC_CONTAINER),
        )))
        .id();
    let Some(parts) = MenuParts::new([holder, viewer]) else {
        commands.entity(holder).despawn();
        return;
    };
    clear_first_slot(player_holders.cursor, &holders, commands.reborrow());
    commands.entity(viewer).open_titled_menu(
        layout,
        parts,
        MenuButtons::default(),
        MenuTitle(title),
    );
    let container_id = packet.container_id.0;
    commands.queue(move |world: &mut World| {
        adopt_holder(world, viewer, holder, container_id);
    });
}

fn adopt_holder(world: &mut World, viewer: Entity, holder: Entity, container_id: i32) {
    let opened = world
        .get::<Menus>(viewer)
        .and_then(Menus::open)
        .filter(|&menu| {
            world
                .get::<MenuParts>(menu)
                .and_then(|parts| parts.holder(MenuLayout::GENERIC_CONTAINER))
                == Some(holder)
        });
    let Some(menu) = opened else {
        if let Ok(entity) = world.get_entity_mut(holder) {
            entity.despawn();
        }
        return;
    };
    if let Ok(mut entity) = world.get_entity_mut(menu) {
        entity.insert(RemoteMenu::opened(container_id, 0));
    }
    if let Ok(mut entity) = world.get_entity_mut(holder) {
        entity.insert(ChildOf(menu));
    }
}
