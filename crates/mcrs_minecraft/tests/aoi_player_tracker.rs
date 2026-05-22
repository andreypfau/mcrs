//! Covers AOI-02. After two players in tracking radius advance through
//! the AoI substrate, each player's `TrackedBy` cache contains the
//! other.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft::world::aoi::TrackedBy;

mod harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn player_tracker_populates_tracked_by_for_in_radius_players() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let a = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));
    let b = spawn_player_in_dim(&mut app, dim, DVec3::new(40.0, 64.0, 0.0));

    // Seed columns around both players so update_own_pov can mirror
    // observers into them. With view-distance 12 and 80-block tracking
    // radius, a generous +/- 14 chunk grid covers everything either
    // player needs.
    seed_columns_in_radius(&mut app, dim, ColumnPos::new(0, 0), 14);
    seed_columns_in_radius(&mut app, dim, ColumnPos::new(2, 0), 14);

    drive_aoi_tick(&mut app);
    nudge_transform(&mut app, a);
    nudge_transform(&mut app, b);
    drive_aoi_tick(&mut app);

    let world = app.world();
    let a_tracked = world.get::<TrackedBy>(a).expect("a has TrackedBy");
    let b_tracked = world.get::<TrackedBy>(b).expect("b has TrackedBy");
    assert!(
        a_tracked.0.contains(&b),
        "A should track B (TrackedBy = {:?})",
        a_tracked.0.as_slice()
    );
    assert!(
        b_tracked.0.contains(&a),
        "B should track A (TrackedBy = {:?})",
        b_tracked.0.as_slice()
    );
}

fn seed_columns_in_radius(app: &mut App, dim: Entity, centre: ColumnPos, radius: i32) {
    let positions: Vec<ColumnPos> = (-radius..=radius)
        .flat_map(|dx| {
            (-radius..=radius).map(move |dz| ColumnPos::new(centre.x + dx, centre.z + dz))
        })
        .collect();
    for pos in positions {
        let exists = app
            .world()
            .get::<ColumnIndex>(dim)
            .map(|idx| idx.0.contains_key(&pos))
            .unwrap_or(false);
        if exists {
            continue;
        }
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

fn nudge_transform(app: &mut App, entity: Entity) {
    let mut t = app
        .world_mut()
        .get_mut::<Transform>(entity)
        .expect("entity has Transform");
    t.translation.x += 0.001;
}
