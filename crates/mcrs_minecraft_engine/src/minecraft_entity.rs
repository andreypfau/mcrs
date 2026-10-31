use crate::bus::to;
use crate::bus::{InboundPlayerPacket, OutboundPlayerPacket, PacketPayload};
use crate::dim_player_plugin::DimPlayerPlugin;
use crate::network_id::network_id;
use crate::session::HostAnchor;
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
use mcrs_minecraft_level::entity::EntityUuid;
use mcrs_minecraft_level::entity::physics::{OldTransform, Transform};
use mcrs_minecraft_level::entity::{EntityNetworkSyncEvent, EntityPlugin};
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundEntityPositionSync;
use mcrs_minecraft_protocol::packets::game::clientbound::PositionPath;

pub struct MinecraftEntityPlugin;

impl Plugin for MinecraftEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EntityPlugin);
        app.add_plugins(DimPlayerPlugin);
        app.add_observer(entity_pos_sync);
        app.add_systems(
            FixedPreUpdate,
            dispatch_inbound_to_dim.after(crate::sub_app_builder::DimInboxDrain),
        );
    }
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
            target: "mcrs_minecraft_engine::bridge",
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
            look: transform.rotation.into(),
            on_ground: true,
        }),
    ));
}
