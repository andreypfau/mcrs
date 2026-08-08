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

/// The per-dim `DimPlayerPlugin::build` body must never register the host-only
/// `spawn_player` login system. `spawn_player` queries `ServerSideConnection`,
/// which is MainWorld-only, so running it inside a DimWorld sub-app is a bug.
///
/// This scans the `DimPlayerPlugin::build` block specifically (not the whole
/// file) so it stays robust to the `Update` vs `bevy_app::Update` spelling and
/// fails if `spawn_player` is reintroduced into the dim plugin set even though
/// other plugins or host-side code may legitimately reference the name.
#[test]
fn spawn_player_not_in_dim_plugin() {
    let source: &str = include_str!("../../mcrs_minecraft/src/world/entity/player/mod.rs");

    let build_body = dim_player_plugin_build_body(source);
    assert!(
        !build_body.contains("spawn_player"),
        "spawn_player must not be registered inside DimPlayerPlugin::build; \
         it is a host-only spawn path (queries ServerSideConnection) that must \
         not run in DimWorld. DimPlayerPlugin::build body was:\n{build_body}"
    );
}

/// Extract the brace-delimited body of `impl Plugin for DimPlayerPlugin`'s
/// `fn build`. Panics if the block cannot be located so the gate fails loudly
/// rather than silently passing if the plugin is renamed or restructured.
fn dim_player_plugin_build_body(source: &str) -> &str {
    let impl_start = source
        .find("impl Plugin for DimPlayerPlugin")
        .expect("DimPlayerPlugin impl must exist in player/mod.rs");
    let after_impl = &source[impl_start..];
    let build_start = after_impl
        .find("fn build")
        .expect("DimPlayerPlugin must have a build fn");
    let body = &after_impl[build_start..];
    let open = body.find('{').expect("build fn must have an opening brace");

    let mut depth = 0usize;
    for (offset, ch) in body[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &body[open..=open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated DimPlayerPlugin::build block");
}
