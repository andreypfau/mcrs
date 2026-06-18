use bevy_ecs::prelude::With;
use mcrs_engine::session::{PlayerSessionCounter, SessionRegistry};
use mcrs_minecraft::configuration::LoadedWorldPreset;
use mcrs_minecraft::world::player_index::PlayerIndex;
use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;
use mcrs_network::ServerSideConnection;

mod common;

#[test]
fn dim_world_contains_no_host_only_resources() {
    let mut app = common::make_host_app();
    common::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert!(!labels.is_empty(), "at least one sub-app must exist after spawn drain");

    for label in &labels {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(label)
            .expect("sub-app present");
        let world = sub_app.world_mut();

        assert!(
            !world.contains_resource::<PlayerIndex>(),
            "PlayerIndex is MainWorld-only and must not appear in DimWorld {label:?}"
        );
        assert!(
            !world.contains_resource::<SessionRegistry>(),
            "SessionRegistry is MainWorld-only and must not appear in DimWorld {label:?}"
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

#[test]
fn spawn_player_not_in_dim_plugin() {
    let source: &str =
        include_str!("../../mcrs_minecraft/src/world/entity/player/mod.rs");
    assert!(
        !source.contains("add_systems(bevy_app::Update, spawn_player)"),
        "spawn_player must not be registered on bevy_app::Update in the dim plugin set; \
         it is a host-only spawn path that must not run in DimWorld"
    );
}
