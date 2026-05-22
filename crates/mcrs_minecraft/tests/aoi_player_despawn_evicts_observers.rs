//! Regression for the per-dim player-removal eviction path. Exercises
//! the production two-world topology: the main App emits an
//! InboundPlayerDespawn via PendingInboundLifecycle; the extract closure
//! shuttles it into the per-dim sub-app's Messages buffer; the per-dim
//! drain system resolves the in-dim Player via HostAnchorRef and retains
//! it out of every column's PlayerObservers.

use bevy_app::{App, AppLabel, FixedPostUpdate, FixedPreUpdate, SubApp};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::PlayerViewDistance;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::aoi::{ChunkSubscriptionSet, PlayerTrackerPlugin, TrackedBy};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, OutboundPlayerPacket, PendingInboundLifecycle,
};
use mcrs_minecraft::world::player_index::HostAnchorRef;

/// Ad-hoc sub-app label for this test.
#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TestDimLabel;

/// Ad-hoc schedule label that chains FixedPreUpdate → FixedPostUpdate,
/// mirroring the DimTick driver used in production sub-apps.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct TestDimTick;

#[test]
fn production_topology_main_world_despawn_evicts_in_dim_observers() {
    // --- Main App setup ---
    let mut main_app = App::new();
    main_app.add_message::<InboundPlayerDespawn>();
    main_app.add_message::<OutboundPlayerPacket>();
    main_app.init_resource::<PendingInboundLifecycle>();

    // Spawn a host_anchor entity in the main world.
    let host_anchor = main_app.world_mut().spawn_empty().id();
    // label_entity == host_anchor for simplicity.
    let label_entity = host_anchor;

    // --- Per-dim SubApp setup ---
    let mut sub_app = SubApp::new();
    sub_app.add_message::<InboundPlayerDespawn>();
    sub_app.add_message::<OutboundPlayerPacket>();

    // Register the schedules that PlayerTrackerPlugin populates.
    sub_app.add_schedule(Schedule::new(FixedPreUpdate));
    sub_app.add_schedule(Schedule::new(FixedPostUpdate));

    // Register the driver schedule.
    sub_app.add_schedule(Schedule::new(TestDimTick));
    sub_app.update_schedule = Some(TestDimTick.intern());

    // The driver system mirrors DimTick in sub_app_builder.rs: run
    // FixedPreUpdate (seeder + drain) then FixedPostUpdate (update_own_pov).
    sub_app.add_systems(TestDimTick, |world: &mut World| {
        world.run_schedule(FixedPreUpdate);
        world.run_schedule(FixedPostUpdate);
    });

    sub_app.add_plugins(PlayerTrackerPlugin);

    // Spawn the Dimension entity in the sub-app world.
    let dim = sub_app.world_mut().spawn(DimensionBundle::default()).id();

    // Spawn the in-dim Player entity inline against SubApp::world_mut().
    let pos = DVec3::new(0.0, 64.0, 0.0);
    let in_dim_player = sub_app
        .world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ChunkSubscriptionSet::default(),
            TrackedBy::default(),
            InDimension(dim),
            HostAnchorRef(host_anchor),
        ))
        .id();

    // Seed a column grid with PlayerObservers pre-attached so the first AoI
    // tick can populate the observer sets without needing mid-tick column
    // spawn (this test focuses on eviction, not the Err-arm race).
    let radius: i32 = 20;
    let columns: Vec<Entity> = {
        let mut entities = Vec::new();
        let mut col_map: rustc_hash::FxHashMap<ColumnPos, ColumnSlot> =
            rustc_hash::FxHashMap::default();
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let pos = ColumnPos::new(dx, dz);
                let column = sub_app
                    .world_mut()
                    .spawn((Column, PlayerObservers::default(), InDimension(dim)))
                    .id();
                entities.push(column);
                col_map.insert(
                    pos,
                    ColumnSlot {
                        entity: column,
                        section_count: 1,
                    },
                );
            }
        }
        sub_app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("DimensionBundle provides ColumnIndex")
            .0
            .extend(col_map);
        entities
    };

    // Wire the extract closure: mirrors sub_app_builder.rs lines 323-343 for
    // InboundPlayerDespawn only. label_entity captured by move.
    sub_app.set_extract(move |main_world, sub_world| {
        let despawns: Vec<InboundPlayerDespawn> = {
            let mut lifecycle = main_world.resource_mut::<PendingInboundLifecycle>();
            let entry = lifecycle.per_dim.entry(label_entity).or_default();
            std::mem::take(&mut entry.despawns)
        };
        if !despawns.is_empty() {
            let mut sub_msgs = sub_world.resource_mut::<Messages<InboundPlayerDespawn>>();
            for msg in despawns {
                sub_msgs.write(msg);
            }
        }
    });

    main_app.insert_sub_app(TestDimLabel, sub_app);

    // --- Tick 1: populate PlayerObservers ---
    // Extract runs (no despawns). Sub: FixedPreUpdate seeds observers and runs
    // the drain (no messages). FixedPostUpdate runs update_own_pov, which
    // populates PlayerObservers for columns within the player's view distance.
    main_app.update();

    let count_before = count_columns_observing_in_sub(&main_app, &columns, in_dim_player);
    assert!(
        count_before > 0,
        "expected at least one column observing the in-dim Player before despawn, got {}",
        count_before
    );

    // --- Simulate main-world disconnect ---
    // Push InboundPlayerDespawn — mirrors process_disconnect in production.
    main_app
        .world_mut()
        .resource_mut::<PendingInboundLifecycle>()
        .per_dim
        .entry(label_entity)
        .or_default()
        .despawns
        .push(InboundPlayerDespawn { host_anchor });

    // --- Tick 2: drain and evict ---
    // Extract shuttles the despawn into sub Messages<InboundPlayerDespawn>.
    // FixedPreUpdate: drain_inbound_player_despawn resolves in_dim_player via
    // HostAnchorRef and retains it out of every column's PlayerObservers.
    main_app.update();

    let count_after = count_columns_observing_in_sub(&main_app, &columns, in_dim_player);
    assert_eq!(
        count_after,
        0,
        "expected zero columns observing the in-dim Player after despawn, got {}",
        count_after
    );
}

fn count_columns_observing_in_sub(app: &App, columns: &[Entity], target: Entity) -> usize {
    let sub = app.get_sub_app(TestDimLabel).expect("sub-app not found");
    let world = sub.world();
    let mut count = 0usize;
    for &column in columns {
        if let Some(obs) = world.get::<PlayerObservers>(column) {
            if obs.0.contains(&target) {
                count += 1;
            }
        }
    }
    count
}
