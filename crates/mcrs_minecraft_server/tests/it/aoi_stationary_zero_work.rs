//! Stationary players cost zero AoI work per tick. A
//! counting system gated like the AoI system records how many times the
//! gate opened; the assertion compares the post-baseline count to the
//! baseline. If the run-criterion gating worked, the gate stays shut on
//! subsequent stationary ticks and the counter stays flat.

use bevy_app::FixedPostUpdate;
use bevy_ecs::prelude::{IntoScheduleConfigs, ResMut, Resource};
use bevy_math::DVec3;
use mcrs_minecraft_level::world::dimension::{DimensionBundle, DimensionId, DimensionTypeConfig};
use mcrs_minecraft_server::world::aoi::{PlayerTrackerSet, on_changed_transform};

use crate::harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[derive(Resource, Default)]
struct GatedRuns(u32);

#[test]
fn stationary_players_trigger_no_aoi_writes() {
    let mut app = make_aoi_app();
    app.init_resource::<GatedRuns>();
    app.add_systems(
        FixedPostUpdate,
        (|mut runs: ResMut<GatedRuns>| runs.0 += 1)
            .in_set(PlayerTrackerSet)
            .run_if(on_changed_transform),
    );
    let dim = app
        .world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new("minecraft:overworld"),
            DimensionTypeConfig::new(-64, 384),
        ))
        .id();
    let _player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Tick 1: the initial Changed<Transform> opens the AoI gate at least
    // once. Capture the count here as the baseline.
    drive_aoi_tick(&mut app);
    let baseline = app.world().resource::<GatedRuns>().0;
    assert!(
        baseline >= 1,
        "baseline should record at least one gated run; got {baseline}"
    );

    // Run 10 stationary ticks. Nothing mutates Transform, so the
    // `on_changed_transform` run-criterion must gate the entire
    // `PlayerTrackerSet` and the counter must remain at baseline.
    for _ in 0..10 {
        drive_aoi_tick(&mut app);
    }

    let after = app.world().resource::<GatedRuns>().0;
    assert_eq!(
        after, baseline,
        "the AoI gate opened on a stationary tick (baseline={baseline}, after={after})"
    );
}
