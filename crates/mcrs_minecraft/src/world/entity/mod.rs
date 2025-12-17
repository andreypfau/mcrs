use crate::world::entity::explosive::primed_tnt::PrimedTntPlugin;
use crate::world::entity::player::PlayerPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{ContainsEntity, On};
use bevy_ecs::system::Query;
use derive_more::{Deref, DerefMut};
use mcrs_engine::entity::physics::{OldTransform, Transform};
use mcrs_engine::entity::{EntityNetworkSyncEvent, EntityPlugin};
use mcrs_engine::world::dimension::InDimension;
use mcrs_network::ServerSideConnection;
use mcrs_protocol::packets::game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    ClientboundMoveEntityRot, ClientboundRotateHead,
};
use mcrs_protocol::uuid::Uuid;
use mcrs_protocol::{ByteAngle, Look, VarInt, WritePacket};
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering::Relaxed;

pub mod attribute;
pub mod explosive;
mod meta;
pub mod player;

pub struct MinecraftEntityPlugin;

pub enum MinecraftEntityType {
    PrimedTnt = 132,
    Player = 155,
}

impl Plugin for MinecraftEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EntityPlugin);
        app.add_plugins(PlayerPlugin);
        app.add_plugins(PrimedTntPlugin);
        app.add_observer(entity_pos_sync);
    }
}

#[derive(Bundle)]
pub struct EntityBundle {
    pub minecraft_entity: MinecraftEntity,
    pub dimension: InDimension,
    pub transform: Transform,
    pub uuid: EntityUuid,
}

impl EntityBundle {
    pub fn new(dimension: InDimension) -> Self {
        Self {
            minecraft_entity: Default::default(),
            dimension,
            transform: Default::default(),
            uuid: Default::default(),
        }
    }

    pub fn with_uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = EntityUuid(uuid);
        self
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
}

#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct MinecraftEntity;

#[derive(Debug, Clone, Copy, Component, Deref, DerefMut)]
pub struct EntityOwner(pub Entity);

#[derive(Debug, Clone, Copy, Component, Deref)]
pub struct EntityUuid(Uuid);

impl Default for EntityUuid {
    fn default() -> Self {
        EntityUuid(Uuid::new_v4())
    }
}

impl ContainsEntity for EntityOwner {
    fn entity(&self) -> Entity {
        self.0
    }
}

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

pub fn entity_pos_sync(
    event: On<EntityNetworkSyncEvent>,
    entity_data: Query<(&Transform, &OldTransform)>,
    mut players: Query<&mut ServerSideConnection>,
) {
    let Ok(mut con) = players.get_mut(event.player) else {
        return;
    };
    let Ok((&transform, &old_transform)) = entity_data.get(event.entity) else {
        return;
    };
    let old_transform = old_transform.0;

    let (y_rot, x_rot, _) = transform.rotation.to_euler(bevy_math::EulerRot::YXZ);
    let y_rot = ByteAngle::from_radians(y_rot);
    let x_rot = ByteAngle::from_radians(x_rot);

    let (old_y_rot, old_x_rot, _) = old_transform.rotation.to_euler(bevy_math::EulerRot::YXZ);

    let old_y_rot = ByteAngle::from_radians(old_y_rot);
    let old_x_rot = ByteAngle::from_radians(old_x_rot);

    let delta = (transform.translation - old_transform.translation) * 4096.0;
    let delta_to_big = delta.x < -32768.0
        || delta.x > 32767.0
        || delta.y < -32768.0
        || delta.y > 32767.0
        || delta.z < -32768.0
        || delta.z > 32767.0;
    let need_sync = transform == old_transform;
    let on_ground = true;
    let entity_id = VarInt(event.entity.index() as i32);
    let pos_changed = delta.length_squared() >= 1.0;
    if delta_to_big || need_sync {
        con.write_packet(&ClientboundEntityPositionSync {
            entity_id,
            position: transform.translation,
            velocity: Default::default(),
            look: Look::from(transform.rotation),
            on_ground,
        });
    } else {
        let rot_changed = y_rot.abs_diff(*old_y_rot) >= 1 || x_rot.abs_diff(*old_x_rot) >= 1;
        if pos_changed && rot_changed {
            con.write_packet(&ClientboundMoveEntityPosRot {
                entity_id,
                delta: delta.to_array().map(|f| f as i16),
                y_rot,
                x_rot,
                on_ground,
            });
        } else if pos_changed {
            con.write_packet(&ClientboundMoveEntityPos {
                entity_id,
                delta: delta.to_array().map(|f| f as i16),
                on_ground,
            });
        } else if rot_changed {
            con.write_packet(&ClientboundMoveEntityRot {
                entity_id,
                y_rot,
                x_rot,
                on_ground,
            });
        }
    }

    if old_y_rot != y_rot {
        con.write_packet(&ClientboundRotateHead {
            entity_id,
            y_head_rot: y_rot,
        })
    }
}
