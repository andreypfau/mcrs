//! Regression for the CR-02 cross-session-leak vector. Spawning a
//! player populates PlayerObservers across the column grid; despawning
//! the player (or removing the Player Component) MUST evict the
//! despawned Entity from every column's PlayerObservers in the same
//! tick, before Bevy can recycle the Entity slot.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};

mod harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn player_despawn_evicts_entity_from_all_column_observers() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Seed a column grid pre-attached with PlayerObservers (this test
    // is about the EVICTION path, not the lifecycle race that
    // aoi_mirror_invariant_under_reconcile_race covers). Pre-seeding
    // lets the first AoI tick populate PlayerObservers fully without
    // needing to exercise the Err-arm fix.
    let columns: Vec<Entity> = {
        let mut entities = Vec::new();
        for dx in -20..=20 {
            for dz in -20..=20 {
                let pos = ColumnPos::new(dx, dz);
                let column = app
                    .world_mut()
                    .spawn((Column, PlayerObservers::default(), InDimension(dim)))
                    .id();
                entities.push(column);
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
            }
        }
        entities
    };

    // Tick 1: update_own_pov populates PlayerObservers for every
    // column within view-distance 12 of the player. Confirm at least
    // one column carries the player in its observer set before we
    // proceed to despawn.
    drive_aoi_tick(&mut app);

    let observed_count_before = count_columns_observing(&app, &columns, player);
    assert!(
        observed_count_before > 0,
        "AoI tick should have populated at least one column's PlayerObservers \
         with the player; got 0. This means the test setup is wrong, not the fix."
    );

    // Despawn the player. The On<Remove, Player> observer fires
    // synchronously in the command-flush and scrubs the Entity from
    // every column's PlayerObservers.
    app.world_mut().despawn(player);

    // The observer fired at despawn-flush time; no extra tick needed.
    // (drive_aoi_tick is still called for symmetry with future tests
    // that may rely on a tick boundary, but the eviction is already
    // complete at this point.)
    drive_aoi_tick(&mut app);

    let observed_count_after = count_columns_observing(&app, &columns, player);
    assert_eq!(
        observed_count_after, 0,
        "after Player despawn, no column's PlayerObservers should still \
         contain the despawned Entity (had {} columns observing before \
         despawn, {} after)",
        observed_count_before, observed_count_after
    );
}

fn count_columns_observing(app: &App, columns: &[Entity], target: Entity) -> usize {
    let world = app.world();
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
