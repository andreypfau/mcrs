use bevy_ecs::entity::Entity;
use bevy_ecs::system::EntityCommands;
use bevy_ecs::world::EntityWorldMut;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory_menu::MenuLayout;

use crate::MenuButtons;
use crate::MenuClosed;
use crate::MenuOf;
use crate::MenuOpened;
use crate::MenuParts;
use crate::MenuTitle;
use crate::Menus;

pub trait MenuCommandsExt {
    fn open_menu(
        &mut self,
        layout: MenuLayout,
        parts: MenuParts,
        buttons: MenuButtons,
    ) -> &mut Self;
    fn open_titled_menu(
        &mut self,
        layout: MenuLayout,
        parts: MenuParts,
        buttons: MenuButtons,
        title: MenuTitle,
    ) -> &mut Self;
    fn close_menu(&mut self) -> &mut Self;
}

impl MenuCommandsExt for EntityCommands<'_> {
    fn open_menu(
        &mut self,
        layout: MenuLayout,
        parts: MenuParts,
        buttons: MenuButtons,
    ) -> &mut Self {
        self.queue_silenced(move |viewer: EntityWorldMut| {
            let id = viewer.id();
            open(viewer.into_world_mut(), id, layout, parts, buttons, None);
        })
    }

    fn open_titled_menu(
        &mut self,
        layout: MenuLayout,
        parts: MenuParts,
        buttons: MenuButtons,
        title: MenuTitle,
    ) -> &mut Self {
        self.queue_silenced(move |viewer: EntityWorldMut| {
            let id = viewer.id();
            open(
                viewer.into_world_mut(),
                id,
                layout,
                parts,
                buttons,
                Some(title),
            );
        })
    }

    fn close_menu(&mut self) -> &mut Self {
        self.queue_silenced(|viewer: EntityWorldMut| {
            let id = viewer.id();
            close(viewer.into_world_mut(), id);
        })
    }
}

fn open(
    world: &mut World,
    viewer: Entity,
    layout: MenuLayout,
    parts: MenuParts,
    buttons: MenuButtons,
    title: Option<MenuTitle>,
) {
    let above_first = world
        .get::<Menus>(viewer)
        .is_some_and(|menus| menus.open() != menus.first());
    if above_first {
        close(world, viewer);
        if world.get_entity(viewer).is_err() {
            return;
        }
    }
    let mut menu = world.spawn((layout, parts, buttons, MenuOf(viewer)));
    if let Some(title) = title {
        menu.insert(title);
    }
    let menu = menu.id();
    let opened = MenuOpened { viewer, menu };
    world.write_message(opened);
    world.trigger(opened);
}

fn close(world: &mut World, viewer: Entity) {
    let Some(menus) = world.get::<Menus>(viewer) else {
        return;
    };
    let Some(menu) = menus.open() else {
        return;
    };
    let first = menus.first();
    let closed = MenuClosed { viewer, menu };
    world.write_message(closed);
    world.trigger(closed);
    if first != Some(menu)
        && let Ok(entity) = world.get_entity_mut(menu)
    {
        entity.despawn();
    }
}
