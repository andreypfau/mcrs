//! Wave 0 failing scaffold — covers AOI-03 plus the mirror-drift
//! pitfall: every entry in a player's ChunkSubscriptionSet must be
//! mirrored by that player's entity living in the corresponding chunk's
//! PlayerObservers, and vice versa.

use bevy_app::App;
use bevy_ecs::prelude::*;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::geometry::ColumnPos;
use mcrs_minecraft::world::aoi::ChunkSubscriptionSet;
use rustc_hash::FxHashMap;

#[derive(Component)]
struct PlayerMarker;

#[derive(Component, Clone, Copy)]
struct ChunkColumn(ColumnPos);

#[test]
fn chunk_subscription_set_mirrors_chunk_player_observers() {
    panic!("not yet implemented — pending AoI system wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();
        let player = app
            .world_mut()
            .spawn((PlayerMarker, ChunkSubscriptionSet::default()))
            .id();
        // Pre-spawn three chunk-column entities at adjacent positions.
        let mut chunk_for_pos: FxHashMap<ColumnPos, Entity> = FxHashMap::default();
        for (x, z) in [(0, 0), (1, 0), (0, 1)] {
            let pos = ColumnPos::new(x, z);
            let e = app
                .world_mut()
                .spawn((ChunkColumn(pos), PlayerObservers::default()))
                .id();
            chunk_for_pos.insert(pos, e);
        }

        // One tick to let update_own_pov populate both sides of the mirror.
        app.update();

        let world = app.world();
        let sub = world
            .get::<ChunkSubscriptionSet>(player)
            .expect("player has ChunkSubscriptionSet");
        for pos in &sub.0 {
            let chunk = chunk_for_pos.get(pos).expect("chunk entity exists");
            let obs = world
                .get::<PlayerObservers>(*chunk)
                .expect("chunk has PlayerObservers");
            assert!(
                obs.0.contains(&player),
                "chunk at {:?} missing player in PlayerObservers",
                pos
            );
        }
        for (pos, chunk) in &chunk_for_pos {
            let obs = world
                .get::<PlayerObservers>(*chunk)
                .expect("chunk has PlayerObservers");
            if obs.0.contains(&player) {
                assert!(
                    sub.0.contains(pos),
                    "PlayerObservers of {:?} contains player but ChunkSubscriptionSet does not",
                    pos
                );
            }
        }
    }
}
