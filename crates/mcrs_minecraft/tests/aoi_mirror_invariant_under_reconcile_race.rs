//! Regression for the column-creation-mid-tick mirror race. The seeder
//! runs in FixedPreUpdate; a column spawned later (in FixedUpdate by
//! reconcile_column_existence, or directly in a test) first reaches
//! update_own_pov in FixedPostUpdate without a PlayerObservers
//! Component. This test asserts update_own_pov establishes the mirror
//! anyway by inserting PlayerObservers via Commands on the Err arm of
//! observers.get_mut — so the AOI-03 mirror invariant holds for the
//! production column-creation lifecycle, not just the test-harness
//! pre-seeded one.

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::aoi::{ChunkSubscriptionSet, TrackedBy};
use mcrs_minecraft::world::bus::{InboundPlayerDespawn, OutboundPlayerPacket};
use rustc_hash::FxHashMap;

mod harness;
use harness::{
    drive_aoi_tick, make_aoi_app, run_fixed_pre_update, spawn_player_in_dim,
    spawn_player_in_dim_with_host_anchor,
};

#[test]
fn mirror_invariant_holds_when_column_lacks_player_observers_at_first_pass() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // CRITICAL: columns are spawned AFTER the seeder (FixedPreUpdate) runs,
    // so they reach update_own_pov (FixedPostUpdate) WITHOUT a PlayerObservers
    // Component. This replicates the production race.
    let columns = drive_aoi_tick_with_mid_tick_column_spawn(
        &mut app,
        dim,
        ColumnPos::new(0, 0),
        20,
    );

    // Tick 1 has run. The Err-arm fix in update_own_pov should have inserted
    // PlayerObservers via Commands for each bare column. Assert the mirror
    // invariant now holds.
    assert_mirror_invariant(&app, player, &columns);

    // Drive a second tick with no player movement. The Player has already
    // converged its ChunkSubscriptionSet on tick 1, so neither the Query
    // filter (Changed<Transform> / Added<ChunkSubscriptionSet>) nor the
    // .run_if(on_changed_transform) gate fires — but the invariant must
    // still hold because tick 1 closed the race.
    drive_aoi_tick(&mut app);
    assert_mirror_invariant(&app, player, &columns);
}

/// Run FixedPreUpdate first (so the seeder runs and sees no columns yet),
/// then spawn columns BARE (no PlayerObservers attached), then run
/// FixedPostUpdate (where update_own_pov hits the Err arm). Returns the
/// spawned column map for assertion.
fn drive_aoi_tick_with_mid_tick_column_spawn(
    app: &mut App,
    dim: Entity,
    centre: ColumnPos,
    radius: i32,
) -> FxHashMap<ColumnPos, Entity> {
    app.world_mut().run_schedule(FixedPreUpdate);
    // Spawn columns AFTER the seeder runs — they reach update_own_pov
    // without PlayerObservers, which is the CR-01 race.
    let columns = seed_bare_column_grid(app, dim, centre, radius);
    app.world_mut().run_schedule(FixedPostUpdate);
    columns
}

fn seed_bare_column_grid(
    app: &mut App,
    dim: Entity,
    centre: ColumnPos,
    radius: i32,
) -> FxHashMap<ColumnPos, Entity> {
    let mut map: FxHashMap<ColumnPos, Entity> = FxHashMap::default();
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let pos = ColumnPos::new(centre.x + dx, centre.z + dz);
            // No PlayerObservers in the spawn bundle — the seeder OR
            // update_own_pov's Err-arm fallback is responsible for attaching it.
            let column = app
                .world_mut()
                .spawn((Column, InDimension(dim)))
                .id();
            let mut col_idx = app
                .world_mut()
                .get_mut::<ColumnIndex>(dim)
                .expect("dimension has ColumnIndex");
            col_idx.0.insert(
                pos,
                ColumnSlot {
                    entity: column,
                    section_count: 1,
                },
            );
            map.insert(pos, column);
        }
    }
    map
}

fn assert_mirror_invariant(
    app: &App,
    player: Entity,
    columns: &FxHashMap<ColumnPos, Entity>,
) {
    let world = app.world();
    let sub = world
        .get::<ChunkSubscriptionSet>(player)
        .expect("player has ChunkSubscriptionSet");
    for pos in sub.0.iter() {
        let column = columns
            .get(pos)
            .copied()
            .unwrap_or_else(|| panic!("subscribed column {:?} not in seed grid", pos));
        let obs = world
            .get::<PlayerObservers>(column)
            .unwrap_or_else(|| {
                panic!(
                    "column at {:?} is missing PlayerObservers Component entirely \
                     — the Err-arm fallback in update_own_pov did not fire",
                    pos
                )
            });
        assert!(
            obs.0.contains(&player),
            "column at {:?} missing player in PlayerObservers (sub.len={})",
            pos,
            sub.0.len()
        );
    }
    for (pos, column) in columns.iter() {
        if let Some(obs) = world.get::<PlayerObservers>(*column)
            && obs.0.contains(&player)
        {
            assert!(
                sub.0.contains(pos),
                "PlayerObservers at {:?} contains player but ChunkSubscriptionSet does not",
                pos
            );
        }
    }
}

#[test]
fn update_own_pov_does_not_panic_when_column_despawns_before_flush() {
    // Replicates a ticket-release race: a bare column lands in the
    // player's desired set during FixedPostUpdate, update_own_pov queues
    // a Commands closure to insert PlayerObservers, and another
    // command-queue entry despawns the column before the closure runs.
    // The fix guards the closure with get_entity_mut so this no-ops
    // instead of panicking.
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    app.world_mut().run_schedule(FixedPreUpdate);
    let columns = seed_bare_column_grid(&mut app, dim, ColumnPos::new(0, 0), 20);

    // Despawn every bare column directly before FixedPostUpdate runs.
    // The closure that update_own_pov queues for any column on the Err
    // arm must observe the despawn (via get_entity_mut) and no-op rather
    // than panic.
    let column_entities: Vec<Entity> = columns.values().copied().collect();
    for column_entity in column_entities {
        app.world_mut().entity_mut(column_entity).despawn();
    }

    app.world_mut().run_schedule(FixedPostUpdate);

    // Smoke: the system completed without panicking and the player's
    // subscription set still converged (the columns are gone from
    // ColumnIndex too but ChunkSubscriptionSet only tracks ColumnPos
    // entries, not their entities).
    let _ = app
        .world()
        .get::<ChunkSubscriptionSet>(player)
        .expect("player still has ChunkSubscriptionSet");
}

#[test]
fn mirror_invariant_holds_with_two_players_same_tick_bare_column() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();

    // Both players at the same position — every column within view-distance-12
    // lands in both desired sets, exercising the multi-insert path on every
    // bare column.
    let player1 = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));
    let player2 = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Spawn bare columns mid-tick (after FixedPreUpdate seeder, before
    // FixedPostUpdate AoI systems) so both players hit the Err arm for the
    // same column entity in the same system run.
    let columns = drive_aoi_tick_with_mid_tick_column_spawn(
        &mut app,
        dim,
        ColumnPos::new(0, 0),
        20,
    );

    // Both players must appear in every shared column's PlayerObservers.
    // If Commands::insert were still used, the second insert at flush time
    // would overwrite the first, causing this assertion to fail for player1.
    assert_mirror_invariant(&app, player1, &columns);
    assert_mirror_invariant(&app, player2, &columns);

    // Explicit per-column check for both players on columns that both
    // subscribed to (i.e., within view distance of both players). Columns
    // outside any player's view-distance circle have no PlayerObservers —
    // the AoI system only touches columns in the desired set.
    let world = app.world();
    let sub1 = world.get::<ChunkSubscriptionSet>(player1).unwrap();
    let sub2 = world.get::<ChunkSubscriptionSet>(player2).unwrap();
    for (pos, column) in columns.iter() {
        if sub1.0.contains(pos) && sub2.0.contains(pos) {
            let obs = world
                .get::<PlayerObservers>(*column)
                .expect("column in both subscription sets missing PlayerObservers");
            assert!(obs.0.contains(&player1), "column {:?} missing player1", pos);
            assert!(obs.0.contains(&player2), "column {:?} missing player2", pos);
        }
    }
}

/// Extends the two-player same-tick bare-column scenario to also drive a player
/// removal and assert eviction composes correctly when both players subscribed to
/// the same bare columns on the same tick (the multi-player same-tick path that
/// produces the Err-arm `commands.queue` composition in update_own_pov).
///
/// After the mirror invariant is established, removes P1 and asserts:
///   1. P2's `TrackedBy` no longer contains P1 (proactive eviction).
///   2. P1's `ChunkSubscriptionSet` and `TrackedBy` are cleared (self-teardown).
///   3. P1 is removed from the shared column's `PlayerObservers`.
#[test]
fn two_player_same_tick_bare_column_removal_evicts_correctly() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();

    // Host anchors live inside this app so the drain's HostAnchorRef lookup
    // resolves unambiguously.
    let ha1 = app.world_mut().spawn_empty().id();
    let ha2 = app.world_mut().spawn_empty().id();

    // Both players at the same position — every bare column within view-distance-12
    // is in both desired sets, exercising the multi-insert Err-arm path.
    let player1 =
        spawn_player_in_dim_with_host_anchor(&mut app, dim, DVec3::new(0.0, 64.0, 0.0), ha1);
    let player2 =
        spawn_player_in_dim_with_host_anchor(&mut app, dim, DVec3::new(0.0, 64.0, 0.0), ha2);

    // Spawn bare columns mid-tick (after FixedPreUpdate seeder, before FixedPostUpdate
    // AoI systems) — this is the same-tick race that exercises the Err-arm path in
    // update_own_pov for both players on each bare column entity.
    let columns = drive_aoi_tick_with_mid_tick_column_spawn(
        &mut app,
        dim,
        ColumnPos::new(0, 0),
        20,
    );

    // Verify the mirror invariant holds after the same-tick bare-column subscription.
    assert_mirror_invariant(&app, player1, &columns);
    assert_mirror_invariant(&app, player2, &columns);

    // Drive a second tick so update_tracked_by can build TrackedBy caches.
    // (Tick 1 ran FixedPostUpdate once; tick 2 runs both schedules again; players
    // haven't moved, so Changed<Transform> may not fire — but they have Added<...>
    // on the first tick from spawn, so tick 1's FixedPostUpdate already ran
    // update_tracked_by. Check precondition.)
    //
    // Non-vacuous precondition: P2.TrackedBy must contain P1 before removal.
    // If this fails, increase ticks or inspect tracking radius.
    let p2_tracked_p1_before = app
        .world()
        .get::<TrackedBy>(player2)
        .map(|tb| tb.0.contains(&player1))
        .unwrap_or(false);
    assert!(
        p2_tracked_p1_before,
        "precondition: P2.TrackedBy must contain P1 after warm-up; \
         check that update_tracked_by ran on the first tick (Changed<Transform> from spawn)"
    );

    // Non-vacuous precondition: P1's ChunkSubscriptionSet must be non-empty.
    let p1_sub_before = app
        .world()
        .get::<ChunkSubscriptionSet>(player1)
        .map(|css| !css.0.is_empty())
        .unwrap_or(false);
    assert!(
        p1_sub_before,
        "precondition: P1's ChunkSubscriptionSet must be non-empty before removal"
    );

    // Clear any warm-up outbound packets (PlayerEnteredView etc.) so assertions
    // below see only eviction-related emits.
    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .for_each(drop);

    // Remove P1 via InboundPlayerDespawn — the same message both the disconnect
    // and transfer-out paths push for the source dim.
    app.world_mut()
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .write(InboundPlayerDespawn { host_anchor: ha1 });

    // Drive only FixedPreUpdate: the drain runs and applies the eviction.
    // FixedPostUpdate (update_own_pov, update_tracked_by) is NOT driven here
    // because neither player moved; the gate would skip them anyway, but
    // isolating to FixedPreUpdate keeps the assertion focused on the drain.
    run_fixed_pre_update(&mut app);

    let world = app.world();

    // Assertion 1: P2's TrackedBy no longer contains P1.
    let p2_tracked_p1_after = world
        .get::<TrackedBy>(player2)
        .map(|tb| tb.0.contains(&player1))
        .unwrap_or(true);
    assert!(
        !p2_tracked_p1_after,
        "P2.TrackedBy still contains P1 after removal via shared drain"
    );

    // Assertion 2a: P1's ChunkSubscriptionSet is cleared.
    let p1_css_empty = world
        .get::<ChunkSubscriptionSet>(player1)
        .map(|css| css.0.is_empty())
        .unwrap_or(true);
    assert!(
        p1_css_empty,
        "P1's ChunkSubscriptionSet is non-empty after removal"
    );

    // Assertion 2b: P1's TrackedBy is cleared.
    let p1_tb_empty = world
        .get::<TrackedBy>(player1)
        .map(|tb| tb.0.is_empty())
        .unwrap_or(true);
    assert!(
        p1_tb_empty,
        "P1's TrackedBy is non-empty after removal"
    );

    // Assertion 3: P1 is removed from every shared column's PlayerObservers.
    let p1_still_observing = columns.values().any(|&column_entity| {
        world
            .get::<PlayerObservers>(column_entity)
            .map(|obs| obs.0.contains(&player1))
            .unwrap_or(false)
    });
    assert!(
        !p1_still_observing,
        "P1 is still present in at least one column's PlayerObservers after removal"
    );

    // Sanity: P2 is still correctly observing its columns (eviction is targeted).
    assert_mirror_invariant(&app, player2, &columns);
}
