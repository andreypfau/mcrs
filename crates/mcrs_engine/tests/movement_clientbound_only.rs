use bevy_ecs::message::Messages;
use mcrs_minecraft::world::bus::OutboundPlayerPacket;
use mcrs_minecraft::world::sub_app_builder::drain_dim_spawn_queue;

mod common;

#[test]
fn teleport_does_not_query_server_side_connection() {
    let source: &str =
        include_str!("../../mcrs_minecraft/src/world/entity/player/movement.rs");
    assert!(
        !source.contains("ServerSideConnection"),
        "movement.rs must not query ServerSideConnection; \
         teleport must emit OutboundPlayerPacket via the message bus instead"
    );
}

#[test]
fn digging_does_not_query_server_side_connection() {
    let source: &str =
        include_str!("../../mcrs_minecraft/src/world/entity/player/digging.rs");
    assert!(
        !source.contains("ServerSideConnection"),
        "digging.rs must not query ServerSideConnection; \
         packet emission must go through OutboundPlayerPacket bus"
    );
}

#[test]
fn game_mode_does_not_query_server_side_connection() {
    let source: &str =
        include_str!("../../mcrs_minecraft/src/world/entity/player/game_mode.rs");
    assert!(
        !source.contains("ServerSideConnection"),
        "game_mode.rs must not query ServerSideConnection; \
         packet emission must go through OutboundPlayerPacket bus"
    );
}

/// Runtime gate: a Changed<Transform> on a dim player must produce at least one
/// OutboundPlayerPacket after the teleport system is rewritten to use the message
/// bus instead of ServerSideConnection. Ignored until that rewrite lands.
#[test]
#[ignore = "teleport system still queries ServerSideConnection; un-ignore after the movement rewrite emits OutboundPlayerPacket"]
fn teleport_emits_outbound_player_packet() {
    use bevy_ecs::prelude::*;
    use mcrs_engine::entity::physics::Transform;
    use mcrs_minecraft::world::entity::player::{HostAnchor, movement::TeleportState};

    let mut app = common::make_host_app();
    common::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 1, "one sub-app after one spawn");
    let label = labels[0];

    let anchor_entity;
    {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(&label)
            .expect("sub-app present");
        let world = sub_app.world_mut();
        let anchor = world.spawn(()).id();
        anchor_entity = anchor;
        world.spawn((
            HostAnchor(anchor),
            Transform::default(),
            TeleportState::default(),
        ));
    }

    {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(&label)
            .expect("sub-app present");
        let world = sub_app.world_mut();
        let mut q = world.query::<(&HostAnchor, &mut Transform)>();
        for (_, mut t) in q.iter_mut(world) {
            let new_pos = t.translation + bevy_math::DVec3::new(1.0, 0.0, 0.0);
            t.translation = new_pos;
        }
    }

    app.update();

    let sub_app = app
        .sub_apps()
        .sub_apps
        .get(&label)
        .expect("sub-app present");
    let msgs = sub_app
        .world()
        .resource::<Messages<OutboundPlayerPacket>>();
    let count = msgs.iter_current_update_messages().count();
    assert!(
        count > 0,
        "teleport system must emit at least one OutboundPlayerPacket when Transform changes; got 0. \
         anchor_entity={anchor_entity:?}"
    );
}
