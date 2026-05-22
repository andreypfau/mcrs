use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::explosive::primed_tnt::PrimedTntPlugin;
use crate::world::entity::player::PlayerPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{ContainsEntity, On};
use bevy_ecs::system::Query;
use derive_more::{Deref, DerefMut};
use mcrs_engine::entity::physics::{OldTransform, Transform};
use mcrs_engine::entity::{EntityNetworkSyncEvent, EntityPlugin};
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::VarInt;
use mcrs_protocol::uuid::Uuid;
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

/// Per-event observer that fans entity-position updates out to a single
/// player via the cross-`World` bus. The downstream bridge layer consumes
/// the typed `PacketPayload::EntityPosSync` variant and translates it to
/// the appropriate wire packet (delta-vs-old, head-rotation, etc.) using
/// per-player wire state it owns.
///
/// Observer system params include `MessageWriter<OutboundPlayerPacket>`,
/// matching the precedent set by other observers on `ReceivedPacketEvent`
/// in `entity/player/movement.rs` and `entity/player/player_action.rs`.
pub fn entity_pos_sync(
    event: On<EntityNetworkSyncEvent>,
    entity_data: Query<(&Transform, &OldTransform)>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((&transform, _old_transform)) = entity_data.get(event.entity) else {
        return;
    };
    // `OldTransform` is still queried so this observer remains gated on
    // the same component shape as before; the actual delta computation
    // moves to the bridge tier where per-player wire state lives.
    let on_ground = true;
    packet_writer.write(OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(event.player),
        priority: PacketPriority::Normal,
        data: PacketPayload::EntityPosSync {
            entity: event.entity,
            position: transform.translation,
            rotation: transform.rotation,
            on_ground,
        },
    });
}
