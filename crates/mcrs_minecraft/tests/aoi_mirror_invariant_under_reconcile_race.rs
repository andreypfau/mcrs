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
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::aoi::ChunkSubscriptionSet;
use rustc_hash::FxHashMap;

mod harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

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
        if let Some(obs) = world.get::<PlayerObservers>(*column) {
            if obs.0.contains(&player) {
                assert!(
                    sub.0.contains(pos),
                    "PlayerObservers at {:?} contains player but ChunkSubscriptionSet does not",
                    pos
                );
            }
        }
    }
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
