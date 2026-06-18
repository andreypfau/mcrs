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
use mcrs_minecraft::world::WorldPlugin;
use mcrs_minecraft::world::bus::OutboundPlayerPacket;
use mcrs_minecraft::world::entity::player::player_action::PlayerWillDestroyBlock;
use mcrs_minecraft_block::block_update::{BlockPlaced, BlockSetRequest};

mod common;

#[test]
fn minecraft_block_plugin_messages_present_in_each_subapp() {
    let mut app = common::make_host_app();
    common::materialise_sub_apps(
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
    let mut app = common::make_host_app();
    common::materialise_sub_apps(&mut app, &[("test:overworld", true)]);

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
    let mut app = common::make_host_app();
    common::materialise_sub_apps(&mut app, &[("test:overworld", true)]);

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

