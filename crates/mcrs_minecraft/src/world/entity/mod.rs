use crate::world::bus::{InboundPlayerPacket, OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::explosive::primed_tnt::PrimedTntPlugin;
use crate::world::entity::player::{HostAnchor, PlayerPlugin};
use bevy_app::{App, FixedPreUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, ContainsEntity, On};
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_math::DVec3;
use derive_more::{Deref, DerefMut};
use mcrs_engine::entity::physics::{OldTransform, Transform};
use mcrs_engine::entity::{EntityNetworkSyncEvent, EntityPlugin};
use mcrs_engine::world::dimension::InDimension;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::uuid::Uuid;
use mcrs_protocol::{Look, VarInt};
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
        app.add_systems(FixedPreUpdate, dispatch_inbound_to_dim);
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

impl From<NetworkEntityId> for i32 {
    fn from(val: NetworkEntityId) -> Self {
        val.0.0
    }
}

/// Per-dim system that re-emits `ReceivedPacketEvent` for every
/// `InboundPlayerPacket` drained from the host→sub-app shuttle.
///
/// The extract closure in `sub_app_builder` drains
/// `PendingInboundPartition.per_dim[label_entity]` into the sub-world's
/// `Messages<InboundPlayerPacket>` buffer each tick. This system consumes
/// that buffer, resolves the in-dim entity via the `HostAnchor` component,
/// and triggers `ReceivedPacketEvent` so all per-dim observers
/// (movement, chat, digging) fire on the correct in-dim entity.
fn dispatch_inbound_to_dim(
    mut reader: MessageReader<InboundPlayerPacket>,
    players: Query<(Entity, &HostAnchor)>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        let Some((in_dim_entity, _)) = players.iter().find(|(_, ha)| ha.0 == msg.player) else {
            continue;
        };
        tracing::debug!(
            target: "mcrs_minecraft::bridge",
            packet_id = msg.id,
            in_dim_entity = ?in_dim_entity,
            "dispatch_inbound_to_dim: routing packet to dim entity"
        );
        commands.trigger(ReceivedPacketEvent {
            entity: in_dim_entity,
            id: msg.id,
            data: msg.data.clone(),
            timestamp: msg.timestamp,
        });
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
    // The `With<OldTransform>` filter encodes the gating invariant in
    // the type system: this observer must only fire on entities that
    // carry both transform components. The actual delta computation
    // moves to the bridge tier where per-player wire state lives.
    entity_data: Query<&Transform, With<OldTransform>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok(transform) = entity_data.get(event.entity) else {
        return;
    };
    packet_writer.write(OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(event.player),
        priority: PacketPriority::Normal,
        data: PacketPayload::EntityPosSync {
            entity_id: event.entity.index_u32() as i32,
            position: transform.translation,
            velocity: DVec3::ZERO,
            look: Look {
                yaw: transform.rotation.y,
                pitch: transform.rotation.x,
            },
            on_ground: true,
        },
    });
    mcrs_network::metrics::BRIDGE_OUTBOUND_MESSAGES_EMITTED_TOTAL.fetch_add(1, Relaxed);
}
