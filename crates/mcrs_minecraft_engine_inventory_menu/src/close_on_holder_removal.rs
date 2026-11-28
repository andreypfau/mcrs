use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Slots;

use crate::MenuCommandsExt;
use crate::MenuOf;
use crate::MenuParts;
use crate::Menus;

pub(crate) fn close_on_holder_removal(
    removed: On<Remove<Slots>>,
    menus: Query<(Entity, &MenuParts, &MenuOf)>,
    viewers: Query<&Menus>,
    mut commands: Commands,
) {
    let holder = removed.event().entity;
    for (menu, parts, &MenuOf(viewer)) in &menus {
        if viewer == holder || !parts.holders().contains(&holder) {
            continue;
        }
        let Ok(viewer_menus) = viewers.get(viewer) else {
            continue;
        };
        if viewer_menus.open() == Some(menu) && viewer_menus.first() != Some(menu) {
            commands.entity(viewer).close_menu();
        }
    }
}
