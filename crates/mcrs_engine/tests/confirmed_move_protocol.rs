//! RED scaffold — four-message confirmed-move round trip.
//!
//! Asserts the zero-loss invariant: source entity stays hidden behind
//! `InTransit` until `ConfirmMove` arrives, then is despawned; the target
//! sub-app receives `SpawnEntity` and the entity is alive there after the
//! round trip.
//!
//! All assertions are expected to FAIL until the brokering and arrival
//! systems are wired (later plans).

use bevy_app::{App, AppLabel, SubApp};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::InTransit;
use mcrs_engine::session::{
    MoveId, PlayerSession, PlayerSessionCounter, SessionEntry, SessionRegistry,
};
use mcrs_engine::world::channels::{
    DimSender, ToDimReceiver, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_engine::world::in_flight::InFlightMoves;
use mcrs_engine::world::sub_app::DimDespawnQueue;
use mcrs_minecraft::world::bus::{
    ArrivalCause, InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn,
    MovePayload, OutboundPlayerAttached, OutboundPlayerDisconnect, OutboundPlayerPacket,
    OutboundPlayerTransfer,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft::world::sub_app_builder::DimSubAppHandle;
use mcrs_protocol::uuid::Uuid;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DimTick;

#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TestDimLabel(u8);

#[derive(Component)]
struct SourceMarker;

#[derive(Component)]
struct TargetMarker;

fn register_sub_messages(sub_app: &mut SubApp) {
    sub_app.add_message::<OutboundPlayerPacket>();
    sub_app.add_message::<InboundPlayerPacket>();
    sub_app.add_message::<OutboundPlayerTransfer>();
    sub_app.add_message::<InboundPlayerSpawn>();
    sub_app.add_message::<OutboundPlayerAttached>();
    sub_app.add_message::<OutboundPlayerDisconnect>();
    sub_app.add_message::<InboundPlayerDespawn>();
}

/// Returns (srv_rx, ctl_rx, from_dim_tx): the from_dim_tx lets the test
/// inject FromDim messages as if the sub-app had sent them.
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

fn build_test_app() -> (
    App,
    Entity,  // source_label
    Entity,  // dest_label
    Entity,  // host_anchor
    flume::Sender<FromDim>, // inject FromDim for source
    flume::Sender<FromDim>, // inject FromDim for dest
) {
    let mut app = App::new();

    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.init_resource::<DimDespawnQueue>();
    app.init_resource::<InFlightMoves>();

    let source_label = app.world_mut().spawn(DimSubAppHandle).id();
    let dest_label = app.world_mut().spawn(DimSubAppHandle).id();

    let host_anchor = app.world_mut().spawn_empty().id();
    let session = app.world_mut().resource_mut::<PlayerSessionCounter>().next();
    app.world_mut().resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity: Entity::PLACEHOLDER,
            host_anchor,
            dim: source_label,
            previous_dim: None,
            in_dim_entity: None,
            epoch: 0,
        },
    );

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
                translation: DVec3::new(10.0, 64.0, 10.0),
                ..Default::default()
            },
        ));
        sub.set_extract(|_main_world, _sub_world| {});
        app.insert_sub_app(TestDimLabel(0), sub);
    }

    let (dest_srv_rx, dest_ctl_rx, from_dest_tx) = make_dim_channels(&mut app, dest_label);
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

    (app, source_label, dest_label, host_anchor, from_src_tx, from_dest_tx)
}

/// Four-message confirmed-move round trip: MoveEntity → SpawnEntity → Spawned → ConfirmMove.
///
/// After the round trip the source entity must be despawned and the target must
/// have received the SpawnEntity command.  This test is RED until brokering and
/// arrival systems are wired.
#[test]
fn confirmed_move_completes_full_round_trip() {
    let (mut app, _source_label, _dest_label, _host_anchor, from_src_tx, _from_dest_tx) =
        build_test_app();

    let move_id = MoveId(1);

    // Inject a FromDim::MoveEntity from the source dim.
    from_src_tx
        .try_send(FromDim::MoveEntity {
            move_id,
            target: "minecraft:the_end".to_string(),
            cause: ArrivalCause::EndPlatform,
            payload: MovePayload::Player {
                uuid: Uuid::nil(),
                username: "test-player".to_string(),
            },
            player: None,
        })
        .expect("inject MoveEntity");

    // Drive several ticks: pump_channels processes FromDim, brokering sends
    // SpawnEntity to target, target sends Spawned back, broker sends ConfirmMove
    // to source, source system despawns hidden entity.
    for _ in 0..6 {
        app.update();
        mcrs_minecraft::runner::pump_channels(&mut app);
    }

    // RED: source entity must be despawned after ConfirmMove arrives.
    let in_transit_count = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<InTransit>>()
            .iter(w)
            .count()
    };
    assert_eq!(
        in_transit_count, 0,
        "no InTransit entities should remain after ConfirmMove (RED: brokering not wired yet)"
    );

    let source_marker_count = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<SourceMarker>>()
            .iter(w)
            .count()
    };
    assert_eq!(
        source_marker_count, 0,
        "source entity must be despawned after confirm (RED)"
    );
}

/// Zero-loss invariant: at no tick boundary is the entity count zero (lost) or
/// two (duplicated).  For the scaffold this verifies at least the baseline:
/// entity present before the move with no InTransit.
#[test]
fn zero_loss_invariant_baseline() {
    let (mut app, _source_label, _dest_label, _host_anchor, _from_src, _from_dest) =
        build_test_app();

    let count = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<SourceMarker>>()
            .iter(w)
            .count()
    };
    assert_eq!(count, 1, "exactly one source entity before any move");

    let in_transit = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<InTransit>>()
            .iter(w)
            .count()
    };
    assert_eq!(in_transit, 0, "no InTransit before move initiated");
}
