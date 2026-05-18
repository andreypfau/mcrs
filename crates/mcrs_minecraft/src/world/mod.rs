use crate::configuration::{LoadedDimensionTypes, LoadedWorldPreset};
use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::explosion::ExplosionPlugin;
use crate::world::loot::LootPlugin;
use mcrs_minecraft_block::block_update::BlockUpdatePlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_state::prelude::OnEnter;
use mcrs_core::AppState;
use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use crate::world::sub_app_builder::DimSubAppHandle;
use tracing::{debug, error, info, warn};

pub mod block;
pub mod bridge;
pub mod bus;
pub mod chunk;
pub mod entity;
pub mod player_index;
mod explosion;
mod format;
mod generate;
mod inventory;
pub mod item;
pub mod loot;
pub mod sub_app_builder;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();

        // Bus + PlayerIndex substrate. Both resources live in the host world.
        // `add_message::<T>()` must run BEFORE any sub-app extract reads
        // `Messages<T>` (the closure panics on `resource_mut` if the
        // double-buffer was never initialised). Pairing with the per-sub-app
        // registrations in `spawn_dim_subapp` is what keeps the contract.
        app.init_resource::<crate::world::player_index::PlayerIndex>();
        app.init_resource::<crate::world::bus::PendingInboundPartition>();
        app.init_resource::<crate::world::bus::PendingInboundLifecycle>();
        app.add_message::<crate::world::bus::OutboundPlayerPacket>();
        app.add_message::<crate::world::bus::InboundPlayerPacket>();
        app.add_message::<crate::world::bus::OutboundPlayerTransfer>();
        app.add_message::<crate::world::bus::InboundPlayerSpawn>();
        app.add_message::<crate::world::bus::OutboundPlayerAttached>();
        app.add_message::<crate::world::bus::OutboundPlayerDisconnect>();
        app.add_message::<crate::world::bus::InboundPlayerDespawn>();
        app.add_systems(
            bevy_app::Update,
            crate::world::bridge::partition_main_inbound,
        );

        // Per-dim plugins composed inside each sub-app via `spawn_dim_subapp`:
        // `DimensionPlugin`, `LightingPlugin`, and `ChunkPlugin` (worldgen).
        // Each of those is self-contained: it reads only the registries the
        // sub-app receives in `DimRegistryBundle` and works against the sub-app
        // World's `Dimension`/`Chunk`/`Column` entities.
        //
        // The other five "host-dead" simulation plugins (`BlockUpdatePlugin`,
        // `MinecraftEntityPlugin`, `MinecraftBlockPlugin`, `ExplosionPlugin`,
        // `LootPlugin`) stay host-side. The shared registries they read
        // (`StaticRegistry<EnchantmentData>`, `TagRegistry<Block>`) are now
        // propagated through `DimRegistryBundle` so per-dim sub-apps see the
        // same frozen registry state as the host, ready for the move-back.
        // Two blockers still keep these plugins on the host: cross-plugin
        // messages (`PlayerWillDestroyBlock`, etc.) emitted host-side but
        // expected by sub-app readers (resolved by a cross-app message bus),
        // and entity ownership for `PlayerPlugin` (resolved by per-dim
        // entity hosting once it lands).
        app.add_plugins(BlockUpdatePlugin);
        app.add_plugins(MinecraftEntityPlugin);
        app.add_plugins(MinecraftBlockPlugin);
        app.add_plugins(ExplosionPlugin);
        app.add_plugins(LootPlugin);
        app.add_observer(
            |trigger: On<Remove, DimSubAppHandle>, mut queue: ResMut<DimDespawnQueue>| {
                queue.0.push(trigger.event().entity);
            },
        );
        app.add_systems(OnEnter(AppState::Playing), enqueue_dim_spawns_from_preset);
    }
}

/// Enqueue one `DimSpawnRequest` per dimension in the loaded world preset.
///
/// Runs at `OnEnter(AppState::Playing)`, after `WorldgenFreeze` has finished
/// loading registries and the world preset. The outer runner loop drains
/// `DimSpawnQueue` immediately after `app.update()` returns, materialising one
/// per-dim sub-app per request.
pub fn enqueue_dim_spawns_from_preset(
    world_preset: Res<LoadedWorldPreset>,
    dimension_types: Res<LoadedDimensionTypes>,
    mut spawn_queue: ResMut<DimSpawnQueue>,
    mut already_enqueued: Local<bool>,
) {
    if *already_enqueued {
        return;
    }

    if !world_preset.is_loaded {
        // OnEnter(Playing) fires after the WorldgenFreeze → Playing transition;
        // by then the preset must be loaded. Treat the unloaded case as an
        // invariant violation and bail without enqueueing — the server will
        // come up with zero dimensions, which makes the failure mode visible.
        error!(
            "LoadedWorldPreset not loaded when OnEnter(AppState::Playing) fired — \
             expected the WorldgenFreeze chain to ensure preset load completion"
        );
        return;
    }

    if world_preset.dimensions.is_empty() {
        warn!(
            "LoadedWorldPreset has no dimensions, enqueueing default overworld spawn request"
        );
        spawn_queue.0.push(DimSpawnRequest {
            dimension_id: DimensionId::new("minecraft:overworld"),
            type_config: DimensionTypeConfig::default(),
            has_sky: true,
        });
        *already_enqueued = true;
        return;
    }

    debug!(
        preset = %world_preset.preset_name,
        dimension_count = world_preset.dimensions.len(),
        "Enqueueing dimension spawn requests from loaded world preset"
    );

    for (dimension_key, dimension_type_ref) in &world_preset.dimensions {
        let resolved = dimension_types
            .0
            .iter()
            .find(|(id, _)| id.as_str() == dimension_type_ref.as_str())
            .map(|(_, dim_type)| {
                (
                    DimensionTypeConfig::new(dim_type.min_y, dim_type.height),
                    dim_type.has_skylight,
                )
            })
            .unwrap_or_else(|| {
                warn!(
                    dimension_type = %dimension_type_ref,
                    dimension_key = %dimension_key,
                    "Dimension type not found, using default config + has_sky=true"
                );
                (DimensionTypeConfig::default(), true)
            });

        debug!(
            dimension_key = %dimension_key,
            dimension_type = %dimension_type_ref,
            min_y = resolved.0.min_y,
            height = resolved.0.height,
            sections = resolved.0.section_count,
            has_skylight = resolved.1,
            "Enqueueing dimension spawn request"
        );

        spawn_queue.0.push(DimSpawnRequest {
            dimension_id: DimensionId::new(dimension_key.as_str()),
            type_config: resolved.0,
            has_sky: resolved.1,
        });
    }

    *already_enqueued = true;
    info!(
        dimension_count = world_preset.dimensions.len(),
        "All dimensions enqueued from world preset"
    );
}
