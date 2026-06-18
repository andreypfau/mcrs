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
use mcrs_core::AppState;
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::snapshot::RegistrySnapshot;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerDisconnect,
    OutboundPlayerPacket,
};
use mcrs_minecraft::world::channel_types::DimChannelsResource;
use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;

/// A two-state stub light table sufficient for sub-app construction. The
/// production table is data-loaded; tests only need the resource to exist.
pub fn make_stub_block_light_table() -> BlockStateLightTable {
    let state_count = 2usize;
    let emission = vec![0u8; state_count].into_boxed_slice();
    let dampening = vec![0u8; state_count].into_boxed_slice();
    let occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); state_count].into_boxed_slice();
    let flags = vec![0u8; state_count].into_boxed_slice();
    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

/// Build a host `App` wired for the production per-dim sub-app builder path.
///
/// Registers the host-side bus messages, channel resource, spawn/despawn
/// queues, registries, and the stub light table that the sub-app extract
/// closure and `pump_channels` read — without them the first `app.update()`
/// after a spawn drain panics. The message set is the union required across
/// the sub-app tests, so callers that need only a subset still get a valid
/// host app.
pub fn make_host_app() -> App {
    let mut app = App::new();
    // ChunkPlugin's worldgen startup uses AssetServer::load, which spawns
    // onto IoTaskPool; TaskPoolPlugin must run first or sub-app Startup panics.
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin::default());
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
    app.insert_resource(RegistryAccess::default());
    app.insert_resource(make_stub_block_light_table());
    app.insert_resource(StaticRegistry::<Block>::new());
    app.insert_resource(StaticRegistry::<EnchantmentData>::default());
    app.insert_resource(TagRegistry::<Block>::default());
    app.insert_resource(RegistrySnapshot::<Biome>::default());

    app
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
            dimension_id: DimensionId::new(id),
            type_config: DimensionTypeConfig::default(),
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
