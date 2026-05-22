//! Wave 0 failing scaffold — covers AOI-07 (TrackedBy update has
//! one-tick latency relative to the observed player's position change).
//! Folia-style asymmetry inherited from schedule placement.

use bevy_app::App;
use bevy_ecs::prelude::*;
use mcrs_minecraft::world::aoi::TrackedBy;

#[derive(Component)]
struct PlayerMarker;

#[test]
fn tracked_by_observes_position_change_with_one_tick_latency() {
    panic!("not yet implemented — pending AoI system wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();
        let a = app
            .world_mut()
            .spawn((PlayerMarker, TrackedBy::default()))
            .id();
        let b = app
            .world_mut()
            .spawn((PlayerMarker, TrackedBy::default()))
            .id();

        // Initial tick to settle.
        app.update();

        // Move A and run tick N. B's TrackedBy must NOT yet reflect the
        // new position-derived membership. (Real Transform writes are
        // wired when the AoI systems land.)
        app.update();
        let b_after_n = app
            .world()
            .get::<TrackedBy>(b)
            .expect("b has TrackedBy")
            .0
            .clone();

        // Tick N+1. B's TrackedBy MUST now reflect A's new position.
        app.update();
        let b_after_n_plus_1 = app
            .world()
            .get::<TrackedBy>(b)
            .expect("b has TrackedBy")
            .0
            .clone();

        assert!(
            !b_after_n.contains(&a) || b_after_n_plus_1.contains(&a),
            "TrackedBy did not exhibit the one-tick latency contract"
        );
    }
}
