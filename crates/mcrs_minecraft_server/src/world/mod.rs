use crate::world_options::DimensionList;
use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_state::prelude::OnEnter;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_engine::dim::DimSubAppHandle;
use mcrs_minecraft_level::world::sub_app::{
    DimDespawnQueue, DimSpawnQueue, DimSpawnRequest, DimensionPlugins,
};
use tracing::{debug, info, warn};

pub mod aoi;
pub mod arrival;
pub mod block_entity;
pub mod block_update;
pub mod chunk;
pub mod entity;
pub mod generate;
#[cfg(test)]
mod generation_tests;
pub mod heightmap;
pub mod light;
pub mod light_codec;
pub mod sub_app_builder;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();
        // Seeded by `DimensionWorldsPlugin` before this plugin loads; the
        // default is for harnesses that compose `WorldPlugin` on its own.
        app.init_resource::<crate::world_options::WorldSeed>();

        // Bus substrate, in the host world.
        // `add_message::<T>()` must run BEFORE any sub-app extract reads
        // `Messages<T>` (the closure panics on `resource_mut` if the
        // double-buffer was never initialised). Pairing with the per-sub-app
        // registrations in `spawn_dim_subapp` is what keeps the contract.
        app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
        app.init_resource::<mcrs_minecraft_engine::channel_types::DimChannelsResource>();
        app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
        app.init_resource::<mcrs_minecraft_network::metrics::BridgeTelemetry>();
        app.init_resource::<mcrs_minecraft_network::metrics::GameDecodeCounts>();
        app.add_message::<mcrs_minecraft_engine::bus::OutboundPlayerPacket>();
        app.add_message::<mcrs_minecraft_engine::bus::InboundPlayerPacket>();
        app.add_message::<mcrs_minecraft_engine::bus::OutboundPlayerAttached>();
        app.add_message::<mcrs_minecraft_engine::bus::OutboundPlayerDisconnect>();
        // `attach_outbound_queue` runs in FixedPreUpdate after the network
        // crate's `spawn_new_raw_connections` system (in NetworkSet::SpawnConnections)
        // has populated the world with new `ServerSideConnection` entities.
        // Commands from `attach_outbound_queue` are flushed at the FixedPreUpdate
        // command-application point, guaranteeing that by FixedPostUpdate
        // every connection entity carries `OutboundQueue`.
        app.add_systems(
            FixedPreUpdate,
            mcrs_minecraft_engine::bridge::attach_outbound_queue
                .after(mcrs_minecraft_network::NetworkSet::SpawnConnections),
        );

        app.add_schedule(bevy_ecs::schedule::Schedule::new(
            mcrs_minecraft_engine::bridge::OutboundFlush,
        ));
        app.add_systems(
            mcrs_minecraft_engine::bridge::OutboundFlush,
            (
                mcrs_minecraft_engine::bridge::bridge_outbound,
                mcrs_minecraft_engine::bridge::dispatch_encode,
            )
                .chain(),
        );
        app.add_systems(
            FixedPostUpdate,
            (
                mcrs_minecraft_engine::bridge::run_outbound_flush
                    .after(bevy_ecs::message::signal_message_update_system),
                mcrs_minecraft_engine::bridge::bridge_inbound,
            )
                .chain(),
        );

        app.add_systems(
            bevy_app::Update,
            (
                mcrs_minecraft_engine::bridge::bridge_inbound_to_channel,
                mcrs_minecraft_engine::bridge::bridge_player_attach,
                mcrs_minecraft_engine::bridge::forward_pending_inbound,
            )
                .chain(),
        );

        app.add_observer(
            |trigger: On<Remove, DimSubAppHandle>, mut queue: ResMut<DimDespawnQueue>| {
                queue.0.push(trigger.event().entity);
            },
        );
        app.add_plugins(mcrs_minecraft_engine::disconnect::DisconnectProtocolPlugin);
        app.add_systems(OnEnter(AppState::Playing), enqueue_dim_spawns);
    }
}

/// Enqueue one `DimSpawnRequest` per baked dimension.
///
/// Runs at `OnEnter(AppState::Playing)`. The outer runner loop drains
/// `DimSpawnQueue` immediately after `app.update()` returns, materialising one
/// per-dim sub-app per request.
pub fn enqueue_dim_spawns(
    dimensions: Res<DimensionList>,
    mut spawn_queue: ResMut<DimSpawnQueue>,
    plugins: Option<Res<DimensionPlugins>>,
    mut already_enqueued: Local<bool>,
) {
    if *already_enqueued {
        return;
    }
    let mut unknown: Vec<_> = plugins
        .iter()
        .flat_map(|plugins| plugins.dimensions())
        .filter(|dimension| !dimensions.iter().any(|(key, _)| key == *dimension))
        .collect();
    unknown.sort_unstable_by_key(|dimension| dimension.as_str());
    unknown.dedup();
    for dimension in unknown {
        warn!(
            %dimension,
            "a plugin is added to a dimension that the world does not spawn at start; it gets the plugin only if it spawns later"
        );
    }

    debug!(
        dimension_count = dimensions.len(),
        "Enqueueing dimension spawn requests"
    );

    for (dimension_key, entry) in dimensions.iter() {
        debug!(
            dimension_key = %dimension_key,
            dimension_type = entry.dimension_type.number(),
            "Enqueueing dimension spawn request"
        );

        spawn_queue.0.push(DimSpawnRequest {
            dimension: dimension_key.clone(),
            dimension_type: entry.dimension_type,
        });
    }

    *already_enqueued = true;
    info!(
        dimension_count = dimensions.len(),
        "All dimensions enqueued"
    );
}
