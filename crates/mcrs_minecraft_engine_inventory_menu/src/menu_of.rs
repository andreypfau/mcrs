use alloc::vec::Vec;

use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[relationship(relationship_target = Menus)]
pub struct MenuOf(pub Entity);

#[derive(Component, Debug, PartialEq, Eq)]
#[relationship_target(relationship = MenuOf, linked_spawn)]
pub struct Menus(Vec<Entity>);

impl Menus {
    #[must_use]
    pub fn open(&self) -> Option<Entity> {
        self.0.last().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }

    pub(crate) fn first(&self) -> Option<Entity> {
        self.0.first().copied()
    }
}
