//! Covers AOI-04 (stationary players cost zero AoI work per tick). The
//! `AoiTickProbe` Resource counts how many times each AoI system body
//! has actually executed; the assertion compares the post-baseline
//! counter to the baseline. If the run-criterion gating worked, neither
//! body fires on subsequent stationary ticks and both counters stay
//! flat.

use bevy_math::DVec3;
use mcrs_engine::world::dimension::DimensionBundle;
use mcrs_minecraft::world::aoi::AoiTickProbe;

mod harness;
use harness::{drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn stationary_players_trigger_no_aoi_writes() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let _player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Tick 1: the player's Added<ChunkSubscriptionSet> + initial
    // Changed<Transform> trigger the AoI systems at least once. Capture
    // the probe state here as the baseline.
    drive_aoi_tick(&mut app);
    let baseline = *app.world().resource::<AoiTickProbe>();
    assert!(
        baseline.own_pov_ran >= 1,
        "baseline should record at least one own_pov body run; got {}",
        baseline.own_pov_ran
    );

    // Run 10 stationary ticks. Nothing mutates Transform, so the
    // `on_changed_transform` run-criterion must gate the entire
    // `PlayerTrackerSet` and the counters must remain at baseline.
    for _ in 0..10 {
        drive_aoi_tick(&mut app);
    }

    let after = *app.world().resource::<AoiTickProbe>();
    assert_eq!(
        after.own_pov_ran, baseline.own_pov_ran,
        "own_pov body executed on a stationary tick (baseline={}, after={})",
        baseline.own_pov_ran, after.own_pov_ran
    );
    assert_eq!(
        after.tracked_by_ran, baseline.tracked_by_ran,
        "tracked_by body executed on a stationary tick (baseline={}, after={})",
        baseline.tracked_by_ran, after.tracked_by_ran
    );
}
