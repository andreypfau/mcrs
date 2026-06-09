//! Own-POV ring expansion is synchronous: a boundary-crossing tick updates the
//! player's `ChunkSubscriptionSet` for every in-range column in the same tick.
//! The chunk bytes themselves are delivered separately by
//! `send_column_queue` once the columns have been generated, so this test
//! asserts the synchronous subscription expansion rather than a wire packet.
//! (`update_own_pov` used to emit a placeholder ChunkLoad with empty bytes
//! here; that crashed real clients decoding zero sections and was removed.)

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::aoi::ChunkSubscriptionSet;

mod harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn own_pov_subscription_expands_synchronously_same_tick() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Subscriptions are only recorded for columns that exist in ColumnIndex;
    // subscribing to ungenerated columns would violate the mirror invariant
    // against PlayerObservers. Seed a grid wide enough to cover the default
    // view-distance Chebyshev square.
    seed_column_grid(&mut app, dim, ColumnPos::new(0, 0), 14);

    // Drive the boundary-crossing tick: the initial spawn qualifies for
    // Added<ChunkSubscriptionSet>, so update_own_pov computes the full
    // subscription set in this tick.
    drive_aoi_tick(&mut app);

    let sub = app
        .world()
        .get::<ChunkSubscriptionSet>(player)
        .expect("player has ChunkSubscriptionSet");
    assert!(
        sub.0.contains(&ColumnPos::new(0, 0)),
        "the player's own column is subscribed on the boundary-crossing tick"
    );
    assert!(
        sub.0.len() > 1,
        "the surrounding ring is subscribed synchronously in the same tick; \
         got {} columns",
        sub.0.len()
    );
}

fn seed_column_grid(app: &mut App, dim: Entity, centre: ColumnPos, radius: i32) {
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            let pos = ColumnPos::new(centre.x + dx, centre.z + dz);
            let column = app
                .world_mut()
                .spawn((Column, PlayerObservers::default(), InDimension(dim)))
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
        }
    }
}
