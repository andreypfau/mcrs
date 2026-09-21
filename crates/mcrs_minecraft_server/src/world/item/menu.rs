use crate::world::item::sync::MenuResync;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{With, Without};
use bevy_ecs::world::World;
use mcrs_minecraft_item::{SlotTable, slots};
use mcrs_minecraft_level::entity::player::Player;

#[derive(Component, Debug)]
pub struct Menu {
    pub container_id: u8,
    pub state_id: u16,
}

impl Menu {
    pub fn next_state_id(&mut self) -> u16 {
        self.state_id = (self.state_id + 1) & 32767;
        self.state_id
    }
}

/// Menu index → (holder, cell).
#[derive(Component, Debug)]
pub struct MenuLayout(pub Vec<(Entity, u16)>);

#[derive(Component, Debug)]
#[relationship(relationship_target = MenusOf)]
pub struct MenuViewer(pub Entity);

#[derive(Component, Debug)]
#[relationship_target(relationship = MenuViewer, linked_spawn)]
pub struct MenusOf(Vec<Entity>);

#[derive(Component, Debug)]
pub struct CurrentMenu(pub Entity);

pub fn open_menus(world: &mut World) {
    let joined: Vec<Entity> = world
        .query_filtered::<Entity, (With<Player>, With<SlotTable>, Without<CurrentMenu>)>()
        .iter(world)
        .collect();
    for player in joined {
        let layout = (0..slots::MENU_COUNT as u16).map(|i| (player, i)).collect();
        let menu = world
            .spawn((
                Menu {
                    container_id: 0,
                    state_id: 0,
                },
                MenuLayout(layout),
                MenuViewer(player),
            ))
            .id();
        world.entity_mut(player).insert(CurrentMenu(menu));
        let mut resync = world.resource_mut::<MenuResync>();
        resync.held_slot.push(player);
        resync.menus_full.push(menu);
    }
}
