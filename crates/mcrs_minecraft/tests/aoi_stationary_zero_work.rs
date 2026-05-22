//! Wave 0 failing scaffold — covers AOI-04 (stationary players cost
//! zero AoI work per tick). The AoiTickProbe Resource counts how many
//! times each AoI system body has actually executed; stationary players
//! must not bump the counter past its tick-1 value across subsequent
//! ticks.

use bevy_app::App;
use bevy_ecs::prelude::*;
use mcrs_minecraft::world::aoi::AoiTickProbe;

#[derive(Component)]
struct PlayerMarker;

#[test]
fn stationary_players_trigger_no_aoi_writes() {
    panic!("not yet implemented — pending AoI system wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();
        app.init_resource::<AoiTickProbe>();

        // Spawn one stationary player.
        let _player = app.world_mut().spawn(PlayerMarker).id();

        // First tick: AoI systems may run because the player was just
        // added. Capture the probe state here.
        app.update();
        let baseline = *app.world().resource::<AoiTickProbe>();

        for _ in 0..10 {
            app.update();
        }

        let after = *app.world().resource::<AoiTickProbe>();
        assert_eq!(
            after.own_pov_ran, baseline.own_pov_ran,
            "own_pov body executed on a stationary tick"
        );
        assert_eq!(
            after.tracked_by_ran, baseline.tracked_by_ran,
            "tracked_by body executed on a stationary tick"
        );
    }
}
