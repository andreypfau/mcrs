use crate::world::entity::player::PlayerPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::VarInt;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;

pub mod attribute;
pub mod explosive;
mod meta;
pub mod player;

pub struct MinecraftEntityPlugin;

impl Plugin for MinecraftEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PlayerPlugin);
    }
}

#[derive(Bundle)]
pub struct EntityBundle {
    pub minecraft_entity: MinecraftEntity,
    pub dimension: InDimension,
    pub transform: Transform,
}

impl EntityBundle {
    pub fn new(dimension: InDimension) -> Self {
        Self {
            minecraft_entity: Default::default(),
            dimension,
            transform: Default::default(),
        }
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
}

#[derive(Component, Default)]
pub struct MinecraftEntity;

#[derive(Debug, Clone, Copy, Component)]
pub struct EntityOwner(pub Option<Entity>);

static ENTITY_ID: AtomicI32 = AtomicI32::new(0);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Component)]
pub struct NetworkEntityId(pub VarInt);

impl Default for NetworkEntityId {
    fn default() -> Self {
        let id = ENTITY_ID.fetch_add(1, Relaxed);
        NetworkEntityId(VarInt(id))
    }
}

impl Into<i32> for NetworkEntityId {
    fn into(self) -> i32 {
        self.0.0
    }
}
