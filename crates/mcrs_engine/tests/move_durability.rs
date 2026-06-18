//! RED scaffold — rollback durability: tick-timeout and dropped-receiver paths.
//!
//! Two tests:
//!  1. `never_ack_dim_triggers_tick_timeout_rollback` — target sub-app has
//!     channels but never drains or processes SpawnEntity.  After N+1 ticks
//!     the source entity must reappear at its original position without
//!     InTransit (rollback).
//!  2. `dropped_receiver_triggers_immediate_rollback` — no target sub-app /
//!     channels at all; send_control_or_teardown sees Disconnected and rolls
//!     back within the same pump_channels call.
//!
//! Both tests are RED until the brokering rollback path is wired (later plans).

use bevy_app::{App, AppLabel, SubApp};
use bevy_ecs::prelude::*;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::InTransit;
use mcrs_engine::session::{
    MoveId, PlayerSessionCounter, SessionEntry, SessionRegistry,
};
use mcrs_engine::world::channels::{
    DimSender, ToDimReceiver, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_engine::world::in_flight::InFlightMoves;
use mcrs_engine::world::sub_app::DimDespawnQueue;
use mcrs_minecraft::world::bus::{
    ArrivalCause, InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn,
    MovePayload, OutboundPlayerAttached, OutboundPlayerDisconnect, OutboundPlayerPacket,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft::world::sub_app_builder::DimSubAppHandle;
use mcrs_protocol::uuid::Uuid;

const SMALL_TIMEOUT: u32 = 3;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DimTick;

#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TestDimLabel(u8);

#[derive(Component)]
struct SourceMarker;

fn register_sub_messages(sub_app: &mut SubApp) {
    sub_app.add_message::<OutboundPlayerPacket>();
    sub_app.add_message::<InboundPlayerPacket>();
    sub_app.add_message::<InboundPlayerSpawn>();
    sub_app.add_message::<OutboundPlayerAttached>();
    sub_app.add_message::<OutboundPlayerDisconnect>();
    sub_app.add_message::<InboundPlayerDespawn>();
}

fn make_dim_channels(
    app: &mut App,
    label_entity: Entity,
) -> (
    flume::Receiver<ToDim>,
    flume::Receiver<ToDim>,
    flume::Sender<FromDim>,
) {
    let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(label_entity, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
    (srv_rx, ctl_rx, from_tx)
}

fn build_source_app() -> (
    App,
    Entity,                 // source_label
    DVec3,                  // initial_translation
    flume::Sender<FromDim>, // inject from source dim
) {
    let mut app = App::new();

    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.init_resource::<DimDespawnQueue>();

    let mut inflight = InFlightMoves::default();
    inflight.timeout_ticks = SMALL_TIMEOUT;
    app.insert_resource(inflight);

    let source_label = app.world_mut().spawn(DimSubAppHandle).id();

    let initial_translation = DVec3::new(5.0, 64.0, 5.0);

    let (srv_rx, ctl_rx, from_src_tx) = make_dim_channels(&mut app, source_label);
    {
        let mut sub = SubApp::new();
        sub.update_schedule = Some(DimTick.intern());
        sub.add_schedule(Schedule::new(DimTick));
        register_sub_messages(&mut sub);
        sub.insert_resource(ToDimReceiver::<ToDim> { serverbound: srv_rx, control: ctl_rx });
        sub.world_mut().spawn((
            SourceMarker,
            Transform {
                translation: initial_translation,
                ..Default::default()
            },
        ));
        sub.set_extract(|_main_world, _sub_world| {});
        app.insert_sub_app(TestDimLabel(0), sub);
    }

    (app, source_label, initial_translation, from_src_tx)
}

/// Test 1: Never-ack dim (tick-timeout rollback).
///
/// The target sub-app has channels registered (so send_control_or_teardown
/// does NOT see Disconnected), but it never drains SpawnEntity.  After
/// SMALL_TIMEOUT + 1 ticks the source entity must reappear with InTransit
/// absent and the same Transform::translation as before.
///
/// RED: brokering rollback not yet wired.
#[test]
fn never_ack_dim_triggers_tick_timeout_rollback() {
    let (mut app, source_label, initial_translation, from_src_tx) = build_source_app();

    // Add a "deaf" target sub-app: channels exist but no drain system runs.
    let dest_label = app.world_mut().spawn(DimSubAppHandle).id();
    let (dest_srv_rx, dest_ctl_rx, _dest_from_tx) = make_dim_channels(&mut app, dest_label);
    {
        let mut sub = SubApp::new();
        sub.update_schedule = Some(DimTick.intern());
        sub.add_schedule(Schedule::new(DimTick));
        register_sub_messages(&mut sub);
        sub.insert_resource(ToDimReceiver::<ToDim> {
            serverbound: dest_srv_rx,
            control: dest_ctl_rx,
        });
        sub.set_extract(|_main_world, _sub_world| {});
        app.insert_sub_app(TestDimLabel(1), sub);
    }

    // Capture source entity id before the move.
    let source_entity = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<SourceMarker>>()
            .single(w)
            .expect("source entity")
    };

    // Inject a MoveEntity from the source dim.
    from_src_tx
        .try_send(FromDim::MoveEntity {
            move_id: MoveId(42),
            target: "minecraft:the_end".to_string(),
            cause: ArrivalCause::EndPlatform,
            payload: MovePayload::Player {
                uuid: Uuid::nil(),
                username: "durability-test".to_string(),
            },
            player: None,
        })
        .expect("inject MoveEntity");

    // Drive SMALL_TIMEOUT + 1 ticks.
    for _ in 0..(SMALL_TIMEOUT + 1) {
        app.update();
        mcrs_minecraft::runner::pump_channels(&mut app);
    }

    // RED: InTransit must be absent after rollback.
    let in_transit_count = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<InTransit>>()
            .iter(w)
            .count()
    };
    assert_eq!(
        in_transit_count, 0,
        "InTransit must be removed after tick-timeout rollback (RED)"
    );

    // Same entity id — not a new spawn.
    let (post_rollback_entity, post_translation) = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        let entity = w
            .query_filtered::<Entity, With<SourceMarker>>()
            .single(w)
            .expect("source entity must still exist after rollback");
        let translation = w.entity(entity).get::<Transform>().expect("Transform").translation;
        (entity, translation)
    };
    assert_eq!(
        post_rollback_entity, source_entity,
        "rollback must restore the SAME entity id, not a new spawn"
    );
    assert_eq!(
        post_translation, initial_translation,
        "entity must be at the same translation after rollback (RED)"
    );
}

/// Test 2: Dropped receiver (Disconnected path, immediate rollback).
///
/// No target sub-app / channels registered for the destination dim.
/// send_control_or_teardown sees Disconnected and must roll back within the
/// same pump_channels call.
///
/// RED: Disconnected rollback not yet wired.
#[test]
fn dropped_receiver_triggers_immediate_rollback() {
    let (mut app, _source_label, initial_translation, from_src_tx) = build_source_app();

    // Deliberately do NOT create a target sub-app or register channels for it.

    // Capture source entity id before the move.
    let source_entity = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<SourceMarker>>()
            .single(w)
            .expect("source entity")
    };

    // Inject a MoveEntity naming a dim that has no registered channels.
    from_src_tx
        .try_send(FromDim::MoveEntity {
            move_id: MoveId(99),
            target: "minecraft:nonexistent_dim".to_string(),
            cause: ArrivalCause::CommandTeleport {
                pos: DVec3::new(0.0, 64.0, 0.0),
            },
            payload: MovePayload::Player {
                uuid: Uuid::nil(),
                username: "disconnected-test".to_string(),
            },
            player: None,
        })
        .expect("inject MoveEntity");

    // One pump_channels call should be enough for immediate rollback on Disconnected.
    app.update();
    mcrs_minecraft::runner::pump_channels(&mut app);

    // RED: InTransit absent and same entity/position.
    let in_transit_count = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<InTransit>>()
            .iter(w)
            .count()
    };
    assert_eq!(
        in_transit_count, 0,
        "InTransit must be removed after Disconnected rollback (RED)"
    );

    let (post_rollback_entity, post_translation) = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        let entity = w
            .query_filtered::<Entity, With<SourceMarker>>()
            .single(w)
            .expect("source entity still exists after rollback");
        let translation = w.entity(entity).get::<Transform>().expect("Transform").translation;
        (entity, translation)
    };
    assert_eq!(
        post_rollback_entity, source_entity,
        "rollback must restore the SAME entity id on Disconnected"
    );
    assert_eq!(
        post_translation, initial_translation,
        "entity must be at the same translation after Disconnected rollback (RED)"
    );
}
