//! Covers AOI-05 (each DimSubApp owns its own AoI substrate; AoI state
//! for a player in dim A does not leak into dim B's `PlayerObservers`
//! or onto dim B's entities). The test materialises two real per-dim
//! sub-apps via the production `spawn_dim_subapp` plumbing (which now
//! installs `PlayerTrackerPlugin` in each), seeds a player + column
//! grid into dim A only, and pumps a few ticks. Dim B's chunk
//! observers must remain empty across the run.

use bevy_app::App;
use bevy_asset::AssetPlugin;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_core::AppState;
use mcrs_core::registry::access::RegistryAccess;
use mcrs_core::registry::snapshot::RegistrySnapshot;
use mcrs_core::registry::static_registry::StaticRegistry;
use mcrs_core::tag::TagRegistry;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::PlayerViewDistance;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionId, DimensionTypeConfig, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_engine::world::sub_app::{DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft::world::aoi::{ChunkSubscriptionSet, TrackedBy};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer,
    PendingInboundLifecycle, PendingInboundPartition,
};
use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_lighting::table::BlockStateLightTable;
use mcrs_vanilla::biome::Biome;
use mcrs_vanilla::block::Block;
use mcrs_vanilla::enchantment::EnchantmentData;

#[test]
fn aoi_state_does_not_leak_across_dim_boundary() {
    let mut app = build_host_app();
    enqueue_dim(&mut app, "test:overworld", true);
    enqueue_dim(&mut app, "test:nether", false);
    drain_dim_spawn_queue(&mut app);

    // Enumerate the per-dim label entities. `app.sub_apps()` exposes
    // the interned labels but not the underlying `Entity` values; the
    // host-world entities carrying `DimSubAppHandle` are the canonical
    // source of label_entity values.
    let label_entities: Vec<Entity> = app
        .world_mut()
        .query::<(Entity, &mcrs_minecraft::world::sub_app_builder::DimSubAppHandle)>()
        .iter(app.world())
        .map(|(e, _)| e)
        .collect();
    assert_eq!(
        label_entities.len(),
        2,
        "expected exactly two sub-app handles in the host world"
    );

    let dim_a_label = label_entities[0];
    let dim_b_label = label_entities[1];

    // Seed dim A with a player + column grid so update_own_pov has
    // something to mirror into.
    seed_player_and_columns(
        app.sub_app_mut(DimAppLabel(dim_a_label)),
        DVec3::new(0.0, 64.0, 0.0),
    );

    // Pump a few ticks. update_own_pov in dim A populates dim A's
    // PlayerObservers; dim B has neither players nor populated columns,
    // so any non-empty PlayerObservers in dim B's world would be a
    // cross-dim leak.
    for _ in 0..3 {
        app.update();
    }

    // Verify dim A's PlayerObservers carry at least one entry (proof
    // that the AoI substrate IS running per-dim — without this the
    // empty assertion below is vacuous).
    let dim_a_observers_total = sum_observer_count(app.sub_app(DimAppLabel(dim_a_label)));
    assert!(
        dim_a_observers_total > 0,
        "dim A's PlayerObservers should be populated; the AoI substrate is not running"
    );

    let dim_b_observers_total = sum_observer_count(app.sub_app(DimAppLabel(dim_b_label)));
    assert_eq!(
        dim_b_observers_total, 0,
        "dim B chunks must have zero PlayerObservers entries (AoI state leaked across dim boundary)"
    );

    // Also assert dim B has zero TrackedBy entries on any entity.
    let dim_b_tracked_by_nonempty =
        nonempty_tracked_by(app.sub_app_mut(DimAppLabel(dim_b_label)));
    assert!(
        !dim_b_tracked_by_nonempty,
        "dim B should not carry any non-empty TrackedBy Components"
    );
}

fn seed_player_and_columns(sub_app: &mut bevy_app::SubApp, player_pos: DVec3) {
    // Find the per-dim Dimension entity (the one carrying ColumnIndex).
    let dim_entity = {
        let mut q = sub_app
            .world_mut()
            .query::<(Entity, &ColumnIndex)>();
        let v: Vec<Entity> = q
            .iter(sub_app.world())
            .map(|(e, _)| e)
            .collect();
        v[0]
    };

    // Seed a 20-radius column grid around the player's position so
    // update_own_pov has columns to mirror into.
    let centre = ColumnPos::from(player_pos);
    let radius = 20i32;
    let positions: Vec<ColumnPos> = (-radius..=radius)
        .flat_map(|dx| {
            (-radius..=radius).map(move |dz| ColumnPos::new(centre.x + dx, centre.z + dz))
        })
        .collect();
    for pos in positions {
        let column = sub_app
            .world_mut()
            .spawn((
                Column,
                PlayerObservers::default(),
                InDimension(dim_entity),
            ))
            .id();
        let mut col_idx = sub_app
            .world_mut()
            .get_mut::<ColumnIndex>(dim_entity)
            .expect("dim entity has ColumnIndex");
        col_idx.0.insert(
            pos,
            ColumnSlot {
                entity: column,
                section_count: 1,
            },
        );
    }

    // Spawn the player with all AoI-relevant Components.
    sub_app.world_mut().spawn((
        Player,
        Transform::from_translation(player_pos),
        PlayerViewDistance::default(),
        ChunkSubscriptionSet::default(),
        TrackedBy::default(),
        InDimension(dim_entity),
    ));
}

fn sum_observer_count(sub_app: &bevy_app::SubApp) -> usize {
    // SubApp does not expose a `&mut World` borrow on its `&` self, so
    // we work via a Query state allocated on the existing `&World`.
    // Bevy 0.18 exposes `World::query::<Q>` on `&mut World`; clone the
    // observers we need into a Vec via the entity ref API on `&World`.
    let world = sub_app.world();
    let component_id = world
        .components()
        .component_id::<PlayerObservers>()
        .expect("PlayerObservers Component must be registered");
    let archetypes = world.archetypes();
    let mut total = 0usize;
    for archetype in archetypes.iter() {
        if !archetype.contains(component_id) {
            continue;
        }
        for archetype_entity in archetype.entities() {
            if let Some(obs) = world.get::<PlayerObservers>(archetype_entity.id()) {
                total += obs.0.len();
            }
        }
    }
    total
}

fn nonempty_tracked_by(sub_app: &mut bevy_app::SubApp) -> bool {
    let mut q = sub_app.world_mut().query::<&TrackedBy>();
    q.iter(sub_app.world()).any(|t| !t.0.is_empty())
}

fn build_host_app() -> App {
    // BEVY_ASSET_ROOT is set in .cargo/config.toml's [env] table so it
    // is in the process environment before any thread starts. No
    // per-test unsafe set_var is needed.

    let mut app = App::new();
    app.add_plugins(bevy_app::TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    // The host needs the bus message registrations + lifecycle
    // resources so the per-dim extract closures (which call
    // resource_mut on main_world.Messages<…> + PendingInbound*) do not
    // panic during sub-app extraction.
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();
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

fn enqueue_dim(app: &mut App, id: &str, sky: bool) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension_id: DimensionId::new(id),
            type_config: DimensionTypeConfig::default(),
            has_sky: sky,
        });
}

fn make_stub_block_light_table() -> BlockStateLightTable {
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

