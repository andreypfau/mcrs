//! Every column a player holds lists the player in its `PlayerObservers`,
//! and no other column does: when the player arrives, when its view moves,
//! and when it goes.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, InDimension,
};
use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft_server::world::entity::player::column_view::ColumnView;
use rustc_hash::FxHashMap;

use crate::harness;
use harness::{columns_in_view, drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn a_column_lists_exactly_the_players_that_hold_it() {
    let mut app = make_aoi_app();
    let dim = app
        .world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new("minecraft:overworld"),
            DimensionTypeConfig::new(-64, 384),
        ))
        .id();
    let player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));
    let columns = seed_column_grid(&mut app, dim, ColumnPos::new(0, 0), 20);

    drive_aoi_tick(&mut app);
    assert_mirror_invariant(&app, player, &columns);

    // Five columns east: the columns left behind must drop the player as the
    // new ones pick it up.
    app.world_mut()
        .entity_mut(player)
        .insert(ColumnView::holding(columns_in_view(DVec3::new(
            5.0 * 16.0,
            64.0,
            0.0,
        ))));
    drive_aoi_tick(&mut app);
    assert_mirror_invariant(&app, player, &columns);

    app.world_mut().entity_mut(player).remove::<ColumnView>();
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

fn assert_mirror_invariant(app: &App, player: Entity, columns: &FxHashMap<ColumnPos, Entity>) {
    let world = app.world();
    let view = world.get::<ColumnView>(player);
    for (pos, column) in columns {
        let listed = world
            .get::<PlayerObservers>(*column)
            .expect("column has PlayerObservers")
            .0
            .contains(&player);
        let held = view.is_some_and(|view| view.holds(*pos));
        assert_eq!(
            listed, held,
            "column {pos:?}: listed on the column {listed}, held by the player {held}"
        );
    }
}
