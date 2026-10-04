use bevy_ecs::prelude::With;
use mcrs_minecraft_level::session::{PlayerSessionCounter, Session};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_server::world::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_server::world_options::LoadedWorldPreset;

use crate::host_app;

#[test]
fn dim_world_contains_no_host_only_resources() {
    let mut app = host_app::make_host_app();
    host_app::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert!(
        !labels.is_empty(),
        "at least one sub-app must exist after spawn drain"
    );

    for label in &labels {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(label)
            .expect("sub-app present");
        let world = sub_app.world_mut();

        let session_count = world
            .query_filtered::<bevy_ecs::entity::Entity, With<Session>>()
            .iter(world)
            .count();
        assert_eq!(
            session_count, 0,
            "sessions are MainWorld-only and must not appear in DimWorld {label:?}"
        );
        assert!(
            !world.contains_resource::<PlayerSessionCounter>(),
            "PlayerSessionCounter is MainWorld-only and must not appear in DimWorld {label:?}"
        );
        assert!(
            !world.contains_resource::<LoadedWorldPreset>(),
            "LoadedWorldPreset is MainWorld-only and must not appear in DimWorld {label:?}"
        );

        let conn_count = world
            .query_filtered::<bevy_ecs::entity::Entity, With<ServerSideConnection>>()
            .iter(world)
            .count();
        assert_eq!(
            conn_count, 0,
            "ServerSideConnection is a MainWorld-only component and must not appear in DimWorld {label:?}"
        );
    }
}

