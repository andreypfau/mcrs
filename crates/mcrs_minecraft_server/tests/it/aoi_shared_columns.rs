//! Players holding the same columns share them: each column lists every one
//! of them, and taking one player away leaves the others listed.

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_minecraft_core::ColumnPos;
use mcrs_minecraft_level::aoi::PlayerObservers;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::world::dimension::{
    DimensionBundle, DimensionId, DimensionTypeConfig, InDimension,
};
use mcrs_minecraft_level::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft_server::world::aoi::TrackedBy;
use mcrs_minecraft_server::world::bus::{InboundPlayerDespawn, OutboundPlayerPacket};
use mcrs_minecraft_server::world::entity::player::column_view::ColumnView;
use rustc_hash::FxHashMap;

use crate::harness;
use harness::{
    columns_in_view, drive_aoi_tick, make_aoi_app, spawn_player_in_dim,
    spawn_player_in_dim_with_host_anchor,
};

fn dimension(app: &mut App) -> Entity {
    app.world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new("minecraft:overworld"),
            DimensionTypeConfig::new(-64, 384),
        ))
        .id()
}

fn origin() -> DVec3 {
    DVec3::new(0.0, 64.0, 0.0)
}

fn listed(app: &App, column: Entity, player: Entity) -> bool {
    app.world()
        .get::<PlayerObservers>(column)
        .expect("column has PlayerObservers")
        .0
        .contains(&player)
}

#[test]
fn every_player_holding_a_column_is_listed_on_it() {
    let mut app = make_aoi_app();
    let dim = dimension(&mut app);
    let first = spawn_player_in_dim(&mut app, dim, origin());
    let second = spawn_player_in_dim(&mut app, dim, origin());
    let columns = seed_column_grid(&mut app, dim, ColumnPos::new(0, 0), 20);

    drive_aoi_tick(&mut app);

    for pos in columns_in_view(origin()) {
        let column = columns[&pos];
        assert!(
            listed(&app, column, first),
            "column {pos:?} missing the first player"
        );
        assert!(
            listed(&app, column, second),
            "column {pos:?} missing the second player"
        );
    }
}

/// Removing one player through `InboundPlayerDespawn` — the message both the
/// disconnect and the transfer-out paths push — takes it off every shared
/// column and out of the other player's `TrackedBy`, and leaves the other
/// player where it was.
#[test]
fn taking_one_player_away_leaves_the_others_listed() {
    let mut app = make_aoi_app();
    let dim = dimension(&mut app);
    let first_anchor = app.world_mut().spawn_empty().id();
    let second_anchor = app.world_mut().spawn_empty().id();
    let first = spawn_player_in_dim_with_host_anchor(&mut app, dim, origin(), first_anchor);
    let second = spawn_player_in_dim_with_host_anchor(&mut app, dim, origin(), second_anchor);
    let columns = seed_column_grid(&mut app, dim, ColumnPos::new(0, 0), 20);

    drive_aoi_tick(&mut app);
    assert!(
        app.world()
            .get::<TrackedBy>(second)
            .is_some_and(|tracked| tracked.0.contains(&first)),
        "precondition: the second player tracks the first"
    );
    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .clear();

    app.world_mut()
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .write(InboundPlayerDespawn {
            host_anchor: first_anchor,
            session: PlayerSession(0),
        });
    drive_aoi_tick(&mut app);

    let world = app.world();
    assert!(
        world
            .get::<TrackedBy>(second)
            .is_some_and(|tracked| !tracked.0.contains(&first)),
        "the second player no longer tracks the first"
    );
    assert!(
        world.get::<ColumnView>(first).is_none(),
        "the first player's view is gone"
    );
    assert!(
        world
            .get::<TrackedBy>(first)
            .is_some_and(|tracked| tracked.0.is_empty()),
        "the first player's TrackedBy is cleared"
    );
    for pos in columns_in_view(origin()) {
        let column = columns[&pos];
        assert!(
            !listed(&app, column, first),
            "column {pos:?} still lists the first player"
        );
        assert!(
            listed(&app, column, second),
            "column {pos:?} lost the second player"
        );
    }
}

/// A view can hold a column whose entity is not there — its sections were
/// never generated in a test, or unloaded under it. The mirror has nothing to
/// list the player on and passes over it.
#[test]
fn a_held_column_without_an_entity_is_passed_over() {
    let mut app = make_aoi_app();
    let dim = dimension(&mut app);
    spawn_player_in_dim(&mut app, dim, origin());

    drive_aoi_tick(&mut app);

    assert!(
        app.world()
            .get::<ColumnIndex>(dim)
            .expect("dimension has ColumnIndex")
            .0
            .is_empty()
    );
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
            app.world_mut()
                .get_mut::<ColumnIndex>(dim)
                .expect("dimension has ColumnIndex")
                .0
                .insert(
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
