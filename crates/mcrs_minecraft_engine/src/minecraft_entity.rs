use crate::bus::InboundPlayerPacket;
use crate::dim_player_plugin::DimPlayerPlugin;
use bevy_app::{App, FixedPreUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, ContainsEntity, on_message};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy_ecs::system::Res;
use derive_more::{Deref, DerefMut};
use mcrs_minecraft_engine_world::entity::EntityPlugin;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::DimPlayerIndex;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_entity_key::EntityUuid;

pub struct MinecraftEntityPlugin;

impl Plugin for MinecraftEntityPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EntityPlugin);
        app.add_plugins(DimPlayerPlugin);
        app.add_systems(
            FixedPreUpdate,
            dispatch_inbound_to_dim
                .run_if(on_message::<InboundPlayerPacket>)
                .in_set(InboundDispatch)
                .after(crate::dim_tick::DimInboxDrain),
        );
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InboundDispatch;

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

fn dispatch_inbound_to_dim(
    mut reader: MessageReader<InboundPlayerPacket>,
    index: Res<DimPlayerIndex>,
    mut commands: Commands,
) {
    for msg in reader.read() {
        let Some(&in_dim_entity) = index.0.get(&msg.session) else {
            continue;
        };
        tracing::debug!(
            target: "mcrs_minecraft_engine::bridge",
            packet_id = msg.id,
            in_dim_entity = ?in_dim_entity,
            "dispatch_inbound_to_dim: routing packet to dim entity"
        );
        if let Some(trigger) = msg.trigger {
            trigger.raise(commands.reborrow(), in_dim_entity, msg.data.clone());
        }
    }
}
