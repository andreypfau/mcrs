use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_protocol::VarInt;
use mcrs_protocol::math::DVec3;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;
use derive_more::Deref;
use crate::world::entity::movement::{entity_changed_chunk, init_old_position, update_old_position};

pub mod attribute;
pub mod player;
pub mod player_action;
mod primed_tnt;
mod meta;
mod movement;

struct EntityPlugin;

impl Plugin for EntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, init_old_position);
        app.add_systems(FixedUpdate, entity_changed_chunk);
        app.add_systems(FixedPostUpdate, update_old_position);
    }
}

#[derive(Debug, Clone, Copy, Component, Deref)]
pub struct OldPosition(pub DVec3);

#[derive(Debug, Clone, Copy, Component)]
pub struct DeltaMovement(pub DVec3);

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
