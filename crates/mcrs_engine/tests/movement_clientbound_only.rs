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

/// Runtime gate: a Changed<Transform> on a dim player must advance
/// TeleportState counters, proving the bus-emit path fired. The
/// flush_from_dim_outbox system drains OutboundPlayerPacket from the sub-app
/// Messages before we can read them here, so we verify via state side-effects
/// (pending_teleports and teleport_id_counter increment) rather than the
/// message buffer.
#[test]
fn teleport_emits_outbound_player_packet() {
    use mcrs_engine::entity::physics::Transform;
    use mcrs_minecraft::world::entity::player::{HostAnchor, movement::TeleportState};

    let mut app = common::make_host_app();
    common::enqueue_spawn(&mut app, "test:overworld", true);
    drain_dim_spawn_queue(&mut app);

    let labels: Vec<_> = app.sub_apps().sub_apps.keys().copied().collect();
    assert_eq!(labels.len(), 1, "one sub-app after one spawn");
    let label = labels[0];

    let player_entity;
    {
        let sub_app = app
            .sub_apps_mut()
            .sub_apps
            .get_mut(&label)
            .expect("sub-app present");
        let world = sub_app.world_mut();
        let anchor = world.spawn(()).id();
        player_entity = world.spawn((
            HostAnchor(anchor),
            Transform::default(),
            TeleportState::default(),
        )).id();
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
    let state = sub_app
        .world()
        .entity(player_entity)
        .get::<TeleportState>()
        .expect("TeleportState still on player entity");
    assert!(
        state.pending_teleports() > 0,
        "teleport system must increment pending_teleports when Transform changes; got 0. \
         player_entity={player_entity:?}"
    );
    assert!(
        state.teleport_id_counter() > 0,
        "teleport system must increment teleport_id_counter when Transform changes; got 0. \
         player_entity={player_entity:?}"
    );
}
