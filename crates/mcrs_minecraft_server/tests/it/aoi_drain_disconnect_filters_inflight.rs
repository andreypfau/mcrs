//! Regression for the deferred-drain bus-filter gap. When
//! drain_pending_disconnects pops a queued host_anchor on tick M, the
//! synchronous on_player_disconnect observer is NOT firing for that
//! anchor this tick — it already fired on the original disconnect tick.
//! Without an explicit push into DisconnectedThisTick at drain time,
//! in-flight bus messages for the queued anchor that land on tick M's
//! bus would slip past filter_inflight_for_disconnect.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use mcrs_minecraft_engine::bus::OutboundPlayerAttached;
use mcrs_minecraft_engine::muxer::disconnect::{
    DisconnectBudget, DisconnectedThisTick, PendingDisconnectQueue,
};
use mcrs_minecraft_engine_world::session::{Place, PlayerSession};

use crate::disconnect_support::{
    build_app, despawn_count, disconnect, insert_player, register_dim_channel, run_drain,
    run_filter,
};

#[test]
fn a_drained_disconnect_drops_its_in_flight_messages_and_the_filter_forgets_it() {
    let mut app = build_app();
    let dim = Entity::from_raw_u32(3).unwrap();
    let host_anchor = app.world_mut().spawn_empty().id();
    let other = app.world_mut().spawn_empty().id();
    let session = insert_player(&mut app, host_anchor, Place::InDim(dim));
    let other_session = insert_player(&mut app, other, Place::InDim(dim));
    app.world_mut().resource_mut::<DisconnectBudget>().0 = 0;
    assert!(
        app.world_mut()
            .resource_mut::<PendingDisconnectQueue>()
            .push_back(host_anchor)
    );

    run_drain(&mut app);
    assert_eq!(
        app.world()
            .resource::<DisconnectedThisTick>()
            .host_anchors
            .as_slice(),
        [host_anchor],
        "the drained anchor is marked as disconnected this tick"
    );

    let mut attached = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>();
    attached.write(OutboundPlayerAttached { session });
    attached.write(OutboundPlayerAttached {
        session: other_session,
    });
    run_filter(&mut app);

    let kept: Vec<PlayerSession> = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>()
        .drain()
        .map(|message| message.session)
        .collect();
    assert_eq!(
        kept,
        [other_session],
        "only the drained anchor's message is dropped"
    );
    assert!(
        app.world()
            .resource::<DisconnectedThisTick>()
            .host_anchors
            .is_empty(),
        "the next tick starts with no disconnected anchor"
    );
}

#[test]
fn a_joining_player_that_disconnects_sends_its_dimension_a_despawn_and_loses_its_anchor() {
    let mut app = build_app();
    let host_anchor = app.world_mut().spawn_empty().id();
    let dim = Entity::from_raw_u32(402).unwrap();
    register_dim_channel(&mut app, dim);
    insert_player(&mut app, host_anchor, Place::Joining(dim));

    disconnect(&mut app, &[host_anchor]);
    run_filter(&mut app);

    assert_eq!(despawn_count(&app, dim), 1);
    assert!(
        app.world().get_entity(host_anchor).is_err(),
        "the session is despawned"
    );
}
