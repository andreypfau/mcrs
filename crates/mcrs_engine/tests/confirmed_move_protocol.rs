use bevy_app::{App, AppLabel, SubApp};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::InTransit;
use mcrs_engine::session::{
    MoveId, PlayerSession, PlayerSessionCounter, SessionEntry, SessionRegistry,
};
use mcrs_engine::world::channels::{
    DimSender, FromDimSender, ToDimReceiver, FROM_DIM_CAPACITY, TO_DIM_CAPACITY,
    TO_DIM_CONTROL_CAPACITY,
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

/// Source-dim system: drain ConfirmMove from the control channel and despawn
/// any SourceMarker entity (simulating despawn-on-confirm).
fn source_drain_confirm(
    rx: Res<ToDimReceiver<ToDim>>,
    markers: Query<Entity, With<SourceMarker>>,
    mut commands: Commands,
) {
    for msg in rx.control.try_iter() {
        if let ToDim::ConfirmMove { .. } = msg {
            for entity in markers.iter() {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Dest-dim system: drain SpawnEntity from the control channel and immediately
/// send Spawned back (simulating successful arrival without running arrival systems).
fn dest_drain_spawn_and_ack(
    rx: Res<ToDimReceiver<ToDim>>,
    sender: Res<FromDimSender<FromDim>>,
) {
    for msg in rx.control.try_iter() {
        if let ToDim::SpawnEntity { move_id, .. } = msg {
            let _ = sender.0.try_send(FromDim::Spawned { move_id });
        }
    }
}

fn build_test_app() -> (
    App,
    Entity,  // source_label
    Entity,  // dest_label
    Entity,  // host_anchor
    PlayerSession, // session
    flume::Sender<FromDim>, // inject FromDim for source
    flume::Sender<FromDim>, // inject FromDim for dest
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
        sub.add_systems(DimTick, source_drain_confirm);
        sub.set_extract(|_main_world, _sub_world| {});
        app.insert_sub_app(TestDimLabel(0), sub);
    }

    let (dest_srv_rx, dest_ctl_rx, from_dest_tx) = make_dim_channels(&mut app, dest_label);
    {
        let (_, dest_from_rx_alias) = {
            // The from_dest_tx is already registered above. Give the dest sub-app
            // a FromDimSender wrapping a clone of the dest sender so it can ack.
            let from_dest_tx_clone = from_dest_tx.clone();
            ((), from_dest_tx_clone)
        };
        let mut sub = SubApp::new();
        sub.update_schedule = Some(DimTick.intern());
        sub.add_schedule(Schedule::new(DimTick));
        register_sub_messages(&mut sub);
        sub.insert_resource(ToDimReceiver::<ToDim> {
            serverbound: dest_srv_rx,
            control: dest_ctl_rx,
        });
        sub.insert_resource(FromDimSender::<FromDim>(
            mcrs_engine::world::channels::DimSender::new(dest_from_rx_alias),
        ));
        sub.add_systems(DimTick, dest_drain_spawn_and_ack);
        sub.set_extract(|_main_world, _sub_world| {});
        app.insert_sub_app(TestDimLabel(1), sub);
    }

    (app, source_label, dest_label, host_anchor, session, from_src_tx, from_dest_tx)
}

/// Four-message confirmed-move round trip: MoveEntity → SpawnEntity → Spawned → ConfirmMove.
///
/// After the round trip the source entity must be despawned and the target must
/// have received the SpawnEntity command.
#[test]
fn confirmed_move_completes_full_round_trip() {
    let (mut app, _source_label, _dest_label, _host_anchor, session, from_src_tx, _from_dest_tx) =
        build_test_app();

    let pre_move_epoch = app
        .world()
        .resource::<SessionRegistry>()
        .get(&session)
        .expect("session exists")
        .epoch;

    // Inject a FromDim::MoveEntity from the source dim, naming the dest by its
    // DimLabel. The test uses a target name that matches dest_label's DimLabel.
    // Since we can't set a DimLabel without going through spawn_dim_subapp, we
    // instead use a name that won't match any label — the host will immediately
    // roll back due to unknown target. To test the happy path, we inject
    // FromDim::MoveEntity with player=Some(session) so the epoch is bumped,
    // then manually inject Spawned to simulate the dest ack.
    //
    // Step 1: Inject MoveEntity (no matching label → host queues rollback).
    // Step 2: Inject Spawned directly to verify the Spawned→ConfirmMove path.

    // For the full happy-path test, we set DimLabel on dest_label so pump_channels
    // can resolve the target name.
    app.world_mut()
        .entity_mut(_dest_label)
        .insert(mcrs_minecraft::world::sub_app_builder::DimLabel(
            "minecraft:the_end".to_string(),
        ));

    from_src_tx
        .try_send(FromDim::MoveEntity {
            move_id: MoveId(0),
            target: "minecraft:the_end".to_string(),
            cause: ArrivalCause::EndPlatform,
            payload: MovePayload::Player {
                uuid: Uuid::nil(),
                username: "test-player".to_string(),
            },
            player: Some(session),
        })
        .expect("inject MoveEntity");

    // Drive 6 ticks: the dest sub-app's DimTick system will drain SpawnEntity
    // and send Spawned back; pump_channels will then send ConfirmMove to source;
    // source sub-app's DimTick system will despawn SourceMarker on ConfirmMove.
    for _ in 0..6 {
        app.update();
        mcrs_minecraft::runner::pump_channels(&mut app);
    }

    // Source entity must be despawned after ConfirmMove arrives.
    let in_transit_count = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<InTransit>>()
            .iter(w)
            .count()
    };
    assert_eq!(in_transit_count, 0, "no InTransit entities should remain after ConfirmMove");

    let source_marker_count = {
        let w = app.sub_app_mut(TestDimLabel(0)).world_mut();
        w.query_filtered::<Entity, With<SourceMarker>>()
            .iter(w)
            .count()
    };
    assert_eq!(source_marker_count, 0, "source entity must be despawned after confirm");

    // Epoch-preservation: the epoch must be pre_move_epoch + 1 after a cross-dim
    // player move.
    let post_move_epoch = app
        .world()
        .resource::<SessionRegistry>()
        .get(&session)
        .expect("session exists")
        .epoch;
    assert_eq!(
        post_move_epoch,
        pre_move_epoch.wrapping_add(1),
        "epoch must be bumped exactly once on a cross-dim player move"
    );
}

/// Zero-loss invariant: at no tick boundary is the entity count zero (lost) or
/// two (duplicated).  Verifies the baseline: entity present before the move
/// with no InTransit.
#[test]
fn zero_loss_invariant_baseline() {
    let (mut app, _source_label, _dest_label, _host_anchor, _session, _from_src, _from_dest) =
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
