//! Shared host-app fixtures for the per-dim sub-app integration tests.
//!
//! Each integration test file (`tests/*.rs`) compiles as its own binary, so
//! a built `App` cannot be shared across tests — but the construction itself
//! was copy-pasted across several files. This module is the single source of
//! truth for that setup: the host `App` wired for the production sub-app
//! builder path, plus the small enqueue/drain helpers the tests drive it with.
//!
//! `BEVY_ASSET_ROOT` is set process-wide in `.cargo/config.toml`, so no
//! per-test env mutation is needed here.

#![allow(dead_code)]

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::light::{BlockLightRegistry, block_light_registry};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_level::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerDisconnect,
    OutboundPlayerPacket,
};
use mcrs_minecraft_server::world::channel_types::DimChannelsResource;
use mcrs_minecraft_server::world::sub_app_builder::drain_dim_spawn_queue;

/// Build a host `App` wired for the production per-dim sub-app builder path.
///
/// Registers the host-side bus messages, channel resource, spawn/despawn
/// queues and registries that the sub-app extract closure and `pump_channels`
/// read — without them the first `app.update()` after a spawn drain panics.
/// The message set is the union required across the sub-app tests, so callers
/// that need only a subset still get a valid host app.
pub fn make_host_app() -> App {
    let mut app = App::new();
    // ChunkPlugin's worldgen startup uses AssetServer::load, which spawns
    // onto IoTaskPool; TaskPoolPlugin must run first or sub-app Startup panics.
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();

    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.init_resource::<DimChannelsResource>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    crate::support::insert_registries(&mut app);

    app
}

/// Give the dimensions spawned from this host a lighting engine. Production
/// inserts the same resource at `Startup`.
pub fn enable_lighting(app: &mut App) {
    let blocks = app.world().resource::<Blocks>().clone();
    app.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}

/// Transition the host app into `AppState::Playing` and run one update so the
/// state change is applied.
pub fn drive_to_playing(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
}

/// Push a single dimension spawn request onto the host spawn queue.
pub fn enqueue_spawn(app: &mut App, id: &str, sky: bool) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension: ResourceKey::from_location(
                ResourceLocation::read(id).expect("a test dimension id is a valid identifier"),
            ),
            type_config: DimensionTypeConfig::new(-64, 384),
            has_sky: sky,
        });
}

/// Enqueue every `(id, has_sky)` pair and drain the queue through the
/// production builder, materialising one sub-app per request.
pub fn materialise_sub_apps(app: &mut App, ids: &[(&str, bool)]) {
    for (id, sky) in ids {
        enqueue_spawn(app, id, *sky);
    }
    drain_dim_spawn_queue(app);
}
