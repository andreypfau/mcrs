use crate::world::entity::player::{DimPlayerPlugin, HostAnchor};
use bevy_app::{App, FixedPreUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::prelude::{Commands, ContainsEntity, On};
use bevy_ecs::query::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Query;
use derive_more::{Deref, DerefMut};
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine::bus::{InboundPlayerPacket, OutboundPlayerPacket, PacketPayload};
use mcrs_minecraft_level::entity::physics::{OldTransform, Transform};
use mcrs_minecraft_level::entity::{EntityNetworkSyncEvent, EntityPlugin};
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_protocol::Look;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityPositionSync;
use mcrs_minecraft_protocol::packets::game::clientbound::PositionPath;

pub mod player;

pub use mcrs_minecraft_level::entity::EntityUuid;

pub struct MinecraftEntityPlugin;

impl Plugin for MinecraftEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EntityPlugin);
        app.add_plugins(DimPlayerPlugin);
        app.add_observer(entity_pos_sync);
        app.add_systems(
            FixedPreUpdate,
            dispatch_inbound_to_dim.after(crate::world::sub_app_builder::DimInboxDrain),
        );
    }
}

/// The id of a player on the wire. It comes from the host anchor, which the player keeps in every
/// dimension (DIM-CHG-001). Players take the odd ids and every other entity the even ones, so the
/// ids of two allocators never meet.
pub fn player_network_id(anchor: Entity) -> i32 {
    (anchor.index_u32().wrapping_mul(2) | 1) as i32
}

/// The id on the wire of `entity`: a player's from its host anchor, any other entity's from
/// itself.
pub fn network_id(entity: Entity, anchor: Option<&HostAnchor>) -> i32 {
    anchor.map_or_else(
        || entity_network_id(entity),
        |anchor| player_network_id(anchor.0),
    )
}

/// The id on the wire of an entity that is not a player: a new one in each dimension it enters
/// (DIM-CHG-009).
fn entity_network_id(entity: Entity) -> i32 {
    entity.index_u32().wrapping_mul(2) as i32
}

#[derive(Bundle)]
pub struct EntityBundle {
    pub minecraft_entity: MinecraftEntity,
    pub dimension: InDimension,
    pub transform: Transform,
    pub uuid: EntityUuid,
}

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

/// Per-dim system that raises the typed event of every `InboundPlayerPacket` drained from the
/// host→sub-app shuttle, through the trigger the packet carries.
///
/// The extract closure in `sub_app_builder` drains
/// `PendingInboundPartition.per_dim[label_entity]` into the sub-world's
/// `Messages<InboundPlayerPacket>` buffer each tick. This system consumes
/// that buffer, resolves the in-dim entity via the `HostAnchor` component,
/// and raises the event so all per-dim observers
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
            target: "mcrs_minecraft_server::bridge",
            packet_id = msg.id,
            in_dim_entity = ?in_dim_entity,
            "dispatch_inbound_to_dim: routing packet to dim entity"
        );
        if let Some(trigger) = msg.trigger {
            trigger.raise(
                &mut commands,
                in_dim_entity,
                msg.data.clone(),
                msg.timestamp,
            );
        }
    }
}

/// Per-event observer that fans entity-position updates out to a single
/// player via the cross-`World` bus. The downstream bridge layer consumes
/// the typed `PacketPayload::EntityPosSync` variant and translates it to
/// the appropriate wire packet (delta-vs-old, head-rotation, etc.) using
/// per-player wire state it owns.
///
pub fn entity_pos_sync(
    event: On<EntityNetworkSyncEvent>,
    // The `With<OldTransform>` filter encodes the gating invariant in
    // the type system: this observer must only fire on entities that
    // carry both transform components. The actual delta computation
    // moves to the bridge tier where per-player wire state lives.
    entity_data: Query<(&Transform, Option<&HostAnchor>), With<OldTransform>>,
    viewers: Query<&HostAnchor>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((transform, anchor)) = entity_data.get(event.entity) else {
        return;
    };
    let Ok(&HostAnchor(viewer)) = viewers.get(event.player) else {
        return;
    };
    let entity_id = network_id(event.entity, anchor);
    packet_writer.write(to(
        viewer,
        PacketPayload::EntityPosSync(ClientboundEntityPositionSync {
            entity_id: VarInt(entity_id),
            position: PositionPath::Linear(transform.translation),
            look: Look {
                yaw: transform.rotation.yaw(),
                pitch: transform.rotation.pitch(),
            },
            on_ground: true,
        }),
    ));
}
