//! Regression for the per-dim player-removal eviction path. Exercises
//! the production two-world topology: the main App emits an
//! InboundPlayerDespawn via PendingInboundLifecycle; the extract closure
//! shuttles it into the per-dim sub-app's Messages buffer; the per-dim
//! drain system resolves the in-dim Player via HostAnchor and retains
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
    InboundPlayerDespawn, OutboundPlayerPacket, PacketPayload, PacketTarget,
    PendingInboundLifecycle,
};
use mcrs_minecraft::world::entity::player::HostAnchor;

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
            HostAnchor(host_anchor),
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
    // HostAnchor and retains it out of every column's PlayerObservers.
    main_app.update();

    let count_after = count_columns_observing_in_sub(&main_app, &columns, in_dim_player);
    assert_eq!(
        count_after,
        0,
        "expected zero columns observing the in-dim Player after despawn, got {}",
        count_after
    );
}

/// Regression for the disconnect removal path proving the three eviction truths:
///   1. Stationary observer O's `TrackedBy` no longer contains T after removal.
///   2. A `PlayerLeftView` packet targeting O carrying T's wire id was emitted.
///   3. T's `ChunkSubscriptionSet` and `TrackedBy` are empty after removal.
///
/// Reuses the same two-world harness (main App + SubApp, `set_extract` test
/// closure) as the existing topology test above. Observer O does NOT move
/// between warm-up and removal — the fix must work for stationary observers.
#[test]
fn disconnect_path_evicts_stationary_observer_three_assertions() {
    // --- Main App setup ---
    let mut main_app = App::new();
    main_app.add_message::<InboundPlayerDespawn>();
    main_app.add_message::<OutboundPlayerPacket>();
    main_app.init_resource::<PendingInboundLifecycle>();

    let host_anchor_o = main_app.world_mut().spawn_empty().id();
    let host_anchor_t = main_app.world_mut().spawn_empty().id();
    // The extract closure keys on label_entity (= the dim label in production).
    // Using host_anchor_o as the label entity is arbitrary; what matters is
    // that both InboundPlayerDespawn pushes use the same key.
    let label_entity = host_anchor_o;

    // --- Per-dim SubApp setup (mirrors the existing test above exactly) ---
    let mut sub_app = SubApp::new();
    sub_app.add_message::<InboundPlayerDespawn>();
    sub_app.add_message::<OutboundPlayerPacket>();
    sub_app.add_schedule(Schedule::new(FixedPreUpdate));
    sub_app.add_schedule(Schedule::new(FixedPostUpdate));
    sub_app.add_schedule(Schedule::new(TestDimTick));
    sub_app.update_schedule = Some(TestDimTick.intern());
    sub_app.add_systems(TestDimTick, |world: &mut World| {
        world.run_schedule(FixedPreUpdate);
        world.run_schedule(FixedPostUpdate);
    });
    sub_app.add_plugins(PlayerTrackerPlugin);

    let dim = sub_app.world_mut().spawn(DimensionBundle::default()).id();

    // Both players at the same position — within the 80-block tracking radius.
    // update_tracked_by will discover each player in the other's column
    // PlayerObservers on tick 1, populating both TrackedBy caches.
    let pos = DVec3::new(0.0, 64.0, 0.0);

    // Observer O: remains stationary throughout; its TrackedBy must be cleared
    // of T after the eviction without O moving.
    let player_o = sub_app
        .world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ChunkSubscriptionSet::default(),
            TrackedBy::default(),
            InDimension(dim),
            HostAnchor(host_anchor_o),
        ))
        .id();

    // Target T: the player to be removed via disconnect.
    let player_t = sub_app
        .world_mut()
        .spawn((
            Player,
            Transform::from_translation(pos),
            PlayerViewDistance::default(),
            ChunkSubscriptionSet::default(),
            TrackedBy::default(),
            InDimension(dim),
            HostAnchor(host_anchor_t),
        ))
        .id();

    // Seed column grid with PlayerObservers pre-attached.
    let radius: i32 = 20;
    let columns: Vec<Entity> = {
        let mut entities = Vec::new();
        let mut col_map: rustc_hash::FxHashMap<ColumnPos, ColumnSlot> =
            rustc_hash::FxHashMap::default();
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let col_pos = ColumnPos::new(dx, dz);
                let column = sub_app
                    .world_mut()
                    .spawn((Column, PlayerObservers::default(), InDimension(dim)))
                    .id();
                entities.push(column);
                col_map.insert(col_pos, ColumnSlot { entity: column, section_count: 1 });
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

    // Extract closure: shuttles InboundPlayerDespawn from PendingInboundLifecycle
    // into the sub-world Messages buffer (identical to the existing test).
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

    // --- Tick 1: warm-up — populate PlayerObservers and TrackedBy ---
    // FixedPreUpdate: insert_player_observers_on_new_columns (no-op; already seeded),
    //   drain_inbound_player_despawn (no messages).
    // FixedPostUpdate: update_own_pov → subscribes both players to their columns,
    //   populating ChunkSubscriptionSet and PlayerObservers;
    //   update_tracked_by → each player discovers the other in neighboring
    //   column PlayerObservers → both TrackedBy caches populated.
    main_app.update();

    // Non-vacuous precondition: O.TrackedBy must contain T before removal.
    // (If this assertion fails, the test cannot meaningfully assert eviction.)
    let o_tracked_t_before = {
        let sub = main_app.get_sub_app(TestDimLabel).expect("sub-app");
        sub.world()
            .get::<TrackedBy>(player_o)
            .map(|tb| tb.0.contains(&player_t))
            .unwrap_or(false)
    };
    assert!(
        o_tracked_t_before,
        "precondition: O.TrackedBy must contain T before removal (got false — \
         the two-player warm-up did not converge; check tracking radius and column seeding)"
    );

    // Non-vacuous precondition: T's ChunkSubscriptionSet must be non-empty.
    let t_sub_non_empty_before = {
        let sub = main_app.get_sub_app(TestDimLabel).expect("sub-app");
        sub.world()
            .get::<ChunkSubscriptionSet>(player_t)
            .map(|css| !css.0.is_empty())
            .unwrap_or(false)
    };
    assert!(
        t_sub_non_empty_before,
        "precondition: T's ChunkSubscriptionSet must be non-empty before removal"
    );

    // Clear any packets emitted during warm-up so post-removal assertions
    // see only the eviction emit.
    {
        let sub = main_app.get_sub_app_mut(TestDimLabel).expect("sub-app");
        sub.world_mut()
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .for_each(drop);
    }

    // --- Simulate disconnect: push InboundPlayerDespawn for T ---
    main_app
        .world_mut()
        .resource_mut::<PendingInboundLifecycle>()
        .per_dim
        .entry(label_entity)
        .or_default()
        .despawns
        .push(InboundPlayerDespawn { host_anchor: host_anchor_t });

    // --- Tick 2: drain and evict (O does NOT move) ---
    // FixedPreUpdate: drain_inbound_player_despawn fires:
    //   emits PlayerLeftView to O, clears T's TrackedBy+ChunkSubscriptionSet,
    //   retains T from every column's PlayerObservers.
    // FixedPostUpdate: on_changed_transform gate fires false (no movement) →
    //   update_own_pov and update_tracked_by are skipped.
    main_app.update();

    // Drain packets from the sub-world (needs &mut World).
    let expected_wire_id = player_t.index_u32() as i32;
    let pkts: Vec<OutboundPlayerPacket> = {
        let sub = main_app.get_sub_app_mut(TestDimLabel).expect("sub-app");
        sub.world_mut()
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect()
    };
    let left_view_for_o = pkts.iter().any(|pkt| {
        matches!(&pkt.target, PacketTarget::SinglePlayer(e) if *e == player_o)
            && matches!(&pkt.data, PacketPayload::PlayerLeftView { entity_ids }
                if entity_ids.contains(&expected_wire_id))
    });
    assert!(
        left_view_for_o,
        "expected PlayerLeftView targeting O with T's wire id; \
         found {} emitted packets (none matched)",
        pkts.len()
    );

    // Read TrackedBy and ChunkSubscriptionSet from the sub-world (immutable).
    let sub = main_app.get_sub_app(TestDimLabel).expect("sub-app");
    let sub_world = sub.world();

    // Assertion 1: O.TrackedBy no longer contains T.
    let o_tracked_t_after = sub_world
        .get::<TrackedBy>(player_o)
        .map(|tb| tb.0.contains(&player_t))
        .unwrap_or(true);
    assert!(
        !o_tracked_t_after,
        "O.TrackedBy still contains T after eviction"
    );

    // Assertion 3: T's ChunkSubscriptionSet and TrackedBy are empty.
    let t_css_empty = sub_world
        .get::<ChunkSubscriptionSet>(player_t)
        .map(|css| css.0.is_empty())
        .unwrap_or(true);
    assert!(
        t_css_empty,
        "T's ChunkSubscriptionSet is non-empty after eviction"
    );
    let t_tracked_by_empty = sub_world
        .get::<TrackedBy>(player_t)
        .map(|tb| tb.0.is_empty())
        .unwrap_or(true);
    assert!(
        t_tracked_by_empty,
        "T's TrackedBy is non-empty after eviction"
    );

    // Bonus: T is also removed from every column's PlayerObservers.
    let count_after = count_columns_observing_in_sub(&main_app, &columns, player_t);
    assert_eq!(
        count_after, 0,
        "expected zero columns observing T after eviction, got {}",
        count_after
    );
}

fn count_columns_observing_in_sub(app: &App, columns: &[Entity], target: Entity) -> usize {
    let sub = app.get_sub_app(TestDimLabel).expect("sub-app not found");
    let world = sub.world();
    let mut count = 0usize;
    for &column in columns {
        if let Some(obs) = world.get::<PlayerObservers>(column)
            && obs.0.contains(&target)
        {
            count += 1;
        }
    }
    count
}
