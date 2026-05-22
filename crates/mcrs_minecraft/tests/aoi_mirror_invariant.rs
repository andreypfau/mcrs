//! Covers AOI-03 plus the mirror-drift pitfall. Every entry in a
//! player's `ChunkSubscriptionSet` must be reflected in the
//! corresponding column's `PlayerObservers`, and vice versa. Boundary
//! crossings exercise both add and remove directions.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::aoi::ChunkSubscriptionSet;
use rustc_hash::FxHashMap;

mod harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn chunk_subscription_set_mirrors_chunk_player_observers() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Seed a column grid large enough to cover both the initial
    // position and the boundary crossings five chunks east; default
    // view distance 12 plus the 5-chunk excursion plus slack gives a
    // safe radius.
    let columns = seed_column_grid(&mut app, dim, ColumnPos::new(0, 0), 20);

    // Tick 1: the player's Added<ChunkSubscriptionSet> filter triggers
    // update_own_pov, which mirrors observers into every newly-added
    // column.
    drive_aoi_tick(&mut app);
    assert_mirror_invariant(&app, player, &columns);

    // Boundary cross: move the player to a different column so a fresh
    // delta runs. Removals MUST also be mirrored.
    {
        let mut t = app
            .world_mut()
            .get_mut::<Transform>(player)
            .expect("player has Transform");
        t.translation.x = 5.0 * 16.0; // five chunks east
    }
    drive_aoi_tick(&mut app);
    assert_mirror_invariant(&app, player, &columns);

    // Boundary cross back; both sides must still agree.
    {
        let mut t = app
            .world_mut()
            .get_mut::<Transform>(player)
            .expect("player has Transform");
        t.translation.x = 0.0;
    }
    drive_aoi_tick(&mut app);
    assert_mirror_invariant(&app, player, &columns);
}

fn seed_column_grid(
    app: &mut App,
    dim: Entity,
    centre: ColumnPos,
    radius: i32,
) -> FxHashMap<ColumnPos, Entity> {
    let mut map: FxHashMap<ColumnPos, Entity> = FxHashMap::default();
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
    // Forward direction: every subscribed column lists the player.
    for pos in sub.0.iter() {
        let column = columns
            .get(pos)
            .copied()
            .unwrap_or_else(|| panic!("subscribed column {:?} not in seed grid", pos));
        let obs = world
            .get::<PlayerObservers>(column)
            .expect("column has PlayerObservers");
        assert!(
            obs.0.contains(&player),
            "column at {:?} missing player in PlayerObservers (sub.len={})",
            pos,
            sub.0.len()
        );
    }
    // Reverse direction: every column that lists the player is in the
    // subscription set.
    for (pos, column) in columns.iter() {
        let obs = world
            .get::<PlayerObservers>(*column)
            .expect("column has PlayerObservers");
        if obs.0.contains(&player) {
            assert!(
                sub.0.contains(pos),
                "PlayerObservers at {:?} contains player but ChunkSubscriptionSet does not",
                pos
            );
        }
    }
}
