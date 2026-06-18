//! Validates the per-dim plugin migration:
//!
//! - `MinecraftBlockPlugin` and `ExplosionPlugin` run inside each
//!   per-dim sub-app, with the per-sub-app `Messages<T>` buffers their
//!   systems read and write registered.
//! - `PlayerWillDestroyBlock` stays within the per-dim sub-app world;
//!   the digging systems write it intra-dim via `MessageWriter` and
//!   `MinecraftBlockPlugin` reads it in the same world.
//! - `BlockUpdatePlugin`, `MinecraftEntityPlugin`, and `LootPlugin`
//!   are now per-dim via the channel migration.

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use mcrs_minecraft::world::WorldPlugin;
use mcrs_minecraft::world::bus::OutboundPlayerPacket;
use mcrs_minecraft::world::entity::player::player_action::PlayerWillDestroyBlock;
use mcrs_minecraft_block::block_update::{BlockPlaced, BlockSetRequest};

mod harness {
    use bevy_app::App;
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
    use mcrs_engine::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
    use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig};
    use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;
    use mcrs_minecraft_lighting::table::BlockStateLightTable;
    use mcrs_vanilla::biome::Biome;
    use mcrs_vanilla::block::Block;
    use mcrs_vanilla::enchantment::EnchantmentData;

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

    pub fn make_main_app_with_minimal_plugins() -> App {
        // BEVY_ASSET_ROOT is set in .cargo/config.toml's [env] table so
        // it is in the process environment before any thread starts.
        // No per-test unsafe set_var is needed.

        let mut app = App::new();
        app.add_plugins(bevy_app::TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(TimePlugin);
        app.insert_resource(Time::<Fixed>::from_hz(20.0));
        app.add_plugins(StatesPlugin);
        app.init_state::<AppState>();
        app.init_resource::<DimSpawnQueue>();
        app.init_resource::<DimDespawnQueue>();
        app.insert_resource(RegistryAccess::default());
        app.insert_resource(make_stub_block_light_table());
        app.insert_resource(StaticRegistry::<Block>::new());
        app.insert_resource(StaticRegistry::<EnchantmentData>::default());
        app.insert_resource(TagRegistry::<Block>::default());
        app.insert_resource(RegistrySnapshot::<Biome>::default());
        app.init_resource::<mcrs_minecraft::world::channel_types::DimChannelsResource>();
        app
    }

    #[allow(dead_code)]
    pub fn drive_to_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Playing);
        app.update();
    }

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

    pub fn materialise_sub_apps(app: &mut App, ids: &[(&str, bool)]) {
        for (id, sky) in ids {
            enqueue_spawn(app, id, *sky);
        }
        drain_dim_spawn_queue(app);
    }
}

#[test]
fn minecraft_block_plugin_messages_present_in_each_subapp() {
    let mut app = harness::make_main_app_with_minimal_plugins();
    harness::materialise_sub_apps(
        &mut app,
        &[("test:overworld", true), ("test:nether", false)],
    );

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 2, "two sub-apps expected");

    for label in &labels {
        let sub_app = app
            .sub_apps()
            .sub_apps
            .get(label)
            .expect("sub-app present");
        let world = sub_app.world();

        // Invariant: per-sub-app registration for PlayerWillDestroyBlock
        // so the extract closure can write into the buffer without
        // panicking on resource_mut.
        assert!(
            world.contains_resource::<Messages<PlayerWillDestroyBlock>>(),
            "Messages<PlayerWillDestroyBlock> missing in sub-app {:?}",
            label
        );
        // Pre-existing OutboundPlayerPacket bus registration must still
        // be in place (defensive regression check).
        assert!(
            world.contains_resource::<Messages<OutboundPlayerPacket>>(),
            "Messages<OutboundPlayerPacket> missing in sub-app {:?}",
            label
        );
    }
}

#[test]
fn explosion_plugin_registered_per_dim_not_host() {
    let mut app = harness::make_main_app_with_minimal_plugins();
    harness::materialise_sub_apps(&mut app, &[("test:overworld", true)]);

    let label = *app
        .sub_apps()
        .sub_apps
        .keys()
        .next()
        .expect("one sub-app expected");
    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label)
        .expect("sub-app present");
    let world = sub_app.world();

    // ExplosionPlugin::tick_explode writes MessageWriter<BlockSetRequest>
    // per-dim. The buffer must exist so the system does not panic on
    // first use. BlockPlaced is registered for symmetry.
    assert!(
        world.contains_resource::<Messages<BlockSetRequest>>(),
        "Messages<BlockSetRequest> missing in per-dim world — \
         tick_explode would panic on MessageWriter<BlockSetRequest>"
    );
    assert!(
        world.contains_resource::<Messages<BlockPlaced>>(),
        "Messages<BlockPlaced> missing in per-dim world"
    );
}


#[test]
fn host_side_no_longer_registers_per_dim_simulation_plugins() {
    // Build a host App with WorldPlugin so we can observe the host-side
    // registrations after the per-dim migration of BlockUpdatePlugin,
    // MinecraftEntityPlugin, and LootPlugin. We do not transition to
    // AppState::Playing; the assertions only inspect resources installed
    // during plugin build().
    let mut app = App::new();
    app.add_plugins(bevy_app::TaskPoolPlugin::default());
    app.add_plugins(bevy_asset::AssetPlugin::default());
    app.add_plugins(bevy_time::TimePlugin);
    app.add_plugins(bevy_state::app::StatesPlugin);
    use bevy_state::app::AppExtStates;
    app.init_state::<mcrs_core::AppState>();
    app.add_plugins(WorldPlugin);

    let world = app.world();

    // BlockUpdatePlugin is no longer host-side: its registrations of
    // Messages<BlockSetRequest> and Messages<BlockPlaced> must NOT be
    // present in the host world. They live in each per-dim sub-app
    // World instead.
    assert!(
        !world.contains_resource::<Messages<BlockSetRequest>>(),
        "BlockUpdatePlugin must no longer register Messages<BlockSetRequest> host-side"
    );
    assert!(
        !world.contains_resource::<Messages<BlockPlaced>>(),
        "BlockUpdatePlugin must no longer register Messages<BlockPlaced> host-side"
    );

    // MinecraftEntityPlugin is no longer host-side: its nested
    // PlayerPlugin -> PlayerActionPlugin chain must NOT have registered
    // Messages<PlayerAction> on the host. The chain runs per-dim.
    use mcrs_minecraft::world::entity::player::player_action::PlayerAction;
    assert!(
        !world.contains_resource::<Messages<PlayerAction>>(),
        "MinecraftEntityPlugin chain (PlayerActionPlugin) must no longer register \
         Messages<PlayerAction> host-side"
    );

    // LootPlugin is no longer host-side: BlockLootTables must NOT be
    // present in the host world.
    use mcrs_minecraft::world::loot::BlockLootTables;
    assert!(
        !world.contains_resource::<BlockLootTables>(),
        "LootPlugin must no longer install BlockLootTables host-side"
    );
}

#[test]
fn per_dim_simulation_plugins_now_in_sub_app() {
    let mut app = harness::make_main_app_with_minimal_plugins();
    harness::materialise_sub_apps(&mut app, &[("test:overworld", true)]);

    let label = *app
        .sub_apps()
        .sub_apps
        .keys()
        .next()
        .expect("one sub-app expected");
    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label)
        .expect("sub-app present");
    let world = sub_app.world();

    // BlockUpdatePlugin now per-dim: Messages<BlockSetRequest> and
    // Messages<BlockPlaced> live in the sub-app World.
    assert!(
        world.contains_resource::<Messages<BlockSetRequest>>(),
        "BlockUpdatePlugin must register Messages<BlockSetRequest> in the per-dim sub-app"
    );
    assert!(
        world.contains_resource::<Messages<BlockPlaced>>(),
        "BlockUpdatePlugin must register Messages<BlockPlaced> in the per-dim sub-app"
    );

    // MinecraftEntityPlugin now per-dim: Messages<PlayerAction> lives in
    // the sub-app World (via PlayerActionPlugin in the PlayerPlugin chain).
    use mcrs_minecraft::world::entity::player::player_action::PlayerAction;
    assert!(
        world.contains_resource::<Messages<PlayerAction>>(),
        "MinecraftEntityPlugin chain must register Messages<PlayerAction> in the per-dim sub-app"
    );

    // LootPlugin now per-dim: BlockLootTables lives in the sub-app World.
    use mcrs_minecraft::world::loot::BlockLootTables;
    assert!(
        world.contains_resource::<BlockLootTables>(),
        "LootPlugin must install BlockLootTables in the per-dim sub-app"
    );
}

