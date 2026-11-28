use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;

use crate::Menus;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[relationship(relationship_target = Menus)]
pub struct MenuOf(pub Entity);
