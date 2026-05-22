//! Wave 0 failing scaffold — covers AOI-02 (PlayerTracker concrete
//! implementation populates each in-radius player's `TrackedBy`).
//! Replaced by a real assertion when the AoI systems land.

use bevy_app::App;
use bevy_ecs::prelude::*;
use mcrs_minecraft::world::aoi::TrackedBy;

#[derive(Component)]
struct PlayerMarker;

#[test]
fn player_tracker_populates_tracked_by_for_in_radius_players() {
    panic!("not yet implemented — pending AoI system wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();

        // Spawn two players within tracking radius of each other.
        let a = app
            .world_mut()
            .spawn((PlayerMarker, TrackedBy::default()))
            .id();
        let b = app
            .world_mut()
            .spawn((PlayerMarker, TrackedBy::default()))
            .id();

        // One tick to give the AoI systems a chance to populate TrackedBy.
        app.update();

        let world = app.world();
        let a_tracked = world.get::<TrackedBy>(a).expect("a has TrackedBy");
        let b_tracked = world.get::<TrackedBy>(b).expect("b has TrackedBy");

        assert!(a_tracked.0.contains(&b), "A should track B");
        assert!(b_tracked.0.contains(&a), "B should track A");
    }
}
