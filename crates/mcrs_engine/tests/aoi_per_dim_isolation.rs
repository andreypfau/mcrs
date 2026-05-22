//! Wave 0 failing scaffold — covers AOI-05 (each DimSubApp owns its
//! own AoI substrate; no AoI Component for a player in dim A appears
//! in dim B's chunks or as a TrackedBy entry on a dim B entity).

use bevy_app::App;
use bevy_ecs::prelude::*;
use mcrs_engine::aoi::PlayerObservers;

#[derive(Component)]
struct PlayerMarker;

#[derive(Component, Clone, Copy)]
struct DimMarker(u8);

#[test]
fn aoi_state_does_not_leak_across_dim_boundary() {
    panic!("not yet implemented — pending AoI system wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();

        // Two dim markers (stand-ins for distinct DimSubApp worlds).
        let _dim_a = app.world_mut().spawn(DimMarker(0)).id();
        let _dim_b = app.world_mut().spawn(DimMarker(1)).id();

        // Spawn a player in dim A only.
        let _player = app.world_mut().spawn(PlayerMarker).id();

        // Spawn one chunk-with-PlayerObservers in each dim.
        let chunk_a = app.world_mut().spawn(PlayerObservers::default()).id();
        let chunk_b = app.world_mut().spawn(PlayerObservers::default()).id();

        // Advance several ticks.
        for _ in 0..5 {
            app.update();
        }

        let world = app.world();
        let _ = world
            .get::<PlayerObservers>(chunk_a)
            .expect("chunk_a has PlayerObservers");
        let obs_b = world
            .get::<PlayerObservers>(chunk_b)
            .expect("chunk_b has PlayerObservers");
        assert!(
            obs_b.0.is_empty(),
            "dim B chunk should have zero PlayerObservers entries (cross-dim leak)"
        );
    }
}
