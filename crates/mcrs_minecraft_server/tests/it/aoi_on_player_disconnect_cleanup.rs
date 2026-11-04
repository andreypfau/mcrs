//! Mid-transit disconnect cleanup — five scenarios covering each tick of
//! the cross-dim transfer choreography. Each scenario stages the
//! relevant state then drives the disconnect protocol directly via
//! `run_system_once` and `Disconnects`.
//!
//! Constructing a real `ServerSideConnection` requires a `RawConnection`
//! socket that integration tests cannot reach, so the tests exercise the
//! protocol pipeline (resources + helpers + filter system) rather than
//! the `On<Remove, ServerSideConnection>` trigger itself. The trigger
//! path is the thin shim documented in `session_lifecycle.rs`.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use mcrs_minecraft_engine::bus::OutboundPlayerAttached;
use mcrs_minecraft_engine_world::session::Place;

use crate::disconnect_support::{
    build_app, despawn_count, disconnect, insert_player, register_dim_channel, run_filter,
};

#[test]
fn disconnect_at_tick_n_e1_4_attached_pending_filter() {
    let mut app = build_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let dim = Entity::from_raw_u32(402).unwrap();
    register_dim_channel(&mut app, dim);
    let session = insert_player(&mut app, host_anchor, Place::Joining(dim));

    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>()
        .write(OutboundPlayerAttached { session });

    disconnect(&mut app, &[host_anchor]);
    run_filter(&mut app);

    let mut attached_msgs = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>();
    let remaining: Vec<_> = attached_msgs.drain().collect();
    assert!(
        remaining.is_empty(),
        "OutboundPlayerAttached filtered, got {}",
        remaining.len()
    );

    assert_eq!(despawn_count(&app, dim), 1);

    assert!(
        app.world().get_entity(host_anchor).is_err(),
        "the session is despawned"
    );
}
