pub mod explosive;
pub mod player;

use bevy_ecs::component::Component;
use bevy_ecs::entity::{ContainsEntity, Entity};
use derive_more::{Deref, DerefMut};
use mcrs_protocol::uuid::Uuid;

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct MinecraftEntity;

#[derive(Debug, Clone, Copy, Component, Deref, DerefMut)]
pub struct EntityOwner(pub Entity);

impl ContainsEntity for EntityOwner {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Component, Deref)]
pub struct EntityUuid(pub Uuid);

impl Default for EntityUuid {
    fn default() -> Self {
        EntityUuid(Uuid::new_v4())
    }
}
