//! Covers AOI-04 (stationary players cost zero AoI work per tick). The
//! `AoiTickProbe` Resource counts how many times the AoI system body
//! has actually executed; the assertion compares the post-baseline
//! counter to the baseline. If the run-criterion gating worked, the
//! body does not fire on subsequent stationary ticks and the counter
//! stays flat.

use bevy_math::DVec3;
use mcrs_minecraft_level::world::dimension::{DimensionBundle, DimensionId, DimensionTypeConfig};
use mcrs_minecraft_server::world::aoi::AoiTickProbe;

use crate::harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn stationary_players_trigger_no_aoi_writes() {
    let mut app = make_aoi_app();
    let dim = app
        .world_mut()
        .spawn(DimensionBundle::new(
            DimensionId::new("minecraft:overworld"),
            DimensionTypeConfig::new(-64, 384),
        ))
        .id();
    let _player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Tick 1: the initial Changed<Transform> triggers the AoI system at
    // least once. Capture the probe state here as the baseline.
    drive_aoi_tick(&mut app);
    let baseline = *app.world().resource::<AoiTickProbe>();
    assert!(
        baseline.tracked_by_ran >= 1,
        "baseline should record at least one tracked_by body run; got {}",
        baseline.tracked_by_ran
    );

    // Run 10 stationary ticks. Nothing mutates Transform, so the
    // `on_changed_transform` run-criterion must gate the entire
    // `PlayerTrackerSet` and the counter must remain at baseline.
    for _ in 0..10 {
        drive_aoi_tick(&mut app);
    }

    let after = *app.world().resource::<AoiTickProbe>();
    assert_eq!(
        after.tracked_by_ran, baseline.tracked_by_ran,
        "tracked_by body executed on a stationary tick (baseline={}, after={})",
        baseline.tracked_by_ran, after.tracked_by_ran
    );
}
