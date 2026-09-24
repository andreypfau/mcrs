//! End-to-end confirmed cross-dim move over the production seam.
//!
//! These tests drive the REAL host broker (`pump_channels`) and the REAL
//! source-dim keep-until-confirm systems (`despawn_on_confirm` /
//! `unhide_on_rollback`) through the production channels, initiating the move
//! exactly as the source does at move-out: stamp the entity `InTransit` with a
//! source-allocated id and emit `FromDim::MoveEntity` carrying that id and the
//! session.
//!
//! They exercise the move-id reconciliation (the host keys its in-flight table
//! on the source's id and echoes it back), the player epoch bump, and both
//! rollback triggers (tick timeout and a disconnected target) — the behaviour
//! the headline feature depends on.

use bevy_app::{App, FixedPreUpdate, FixedUpdate};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use bevy_math::DVec3;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::{Despawned, InTransit};
use mcrs_minecraft_level::session::{MoveId, Place, PlayerSession, SessionPlacement};
use mcrs_minecraft_level::world::channels::{
    FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY, ToDimReceiver,
};
use mcrs_minecraft_level::world::in_flight::{InFlightMoves, MoveIds};
use mcrs_minecraft_level::world::sub_app::DimDespawnQueue;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::dim::{expire_moves, pump_channels};
use mcrs_minecraft_server::world::bus::InboundEntitySpawn;
use mcrs_minecraft_server::world::bus::{
    ArrivalCause, InboundConfirmMove, InboundRollbackMove, MovePayload, OutboundPlayerPacket,
};
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_minecraft_server::world::entity::player::{despawn_on_confirm, unhide_on_rollback};
use mcrs_minecraft_server::world::session::SessionBundle;
use mcrs_minecraft_server::world::sub_app_builder::{DimLabel, DimSubAppHandle};

const SOURCE_NAME: &str = "minecraft:overworld";
const DEST_NAME: &str = "minecraft:the_nether";
const SESSION: PlayerSession = PlayerSession(42);
const START_POS: DVec3 = DVec3::new(10.0, 64.0, 20.0);

/// Host endpoints plus the raw channel handles each side holds.
struct Harness {
    host: App,
    source_label: Entity,
    dest_label: Entity,
    session_anchor: Entity,
    /// The source dim sends its move request through this.
    source_from_tx: flume::Sender<FromDim>,
    /// The source dim's control receiver (confirm / rollback land here).
    source_ctl_rx: flume::Receiver<ToDim>,
    /// The target dim acks `Spawned` through this.
    dest_from_tx: flume::Sender<FromDim>,
    /// The target dim's control receiver (the spawn command lands here).
    dest_ctl_rx: Option<flume::Receiver<ToDim>>,
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
        .insert(label_entity, srv_tx, ctl_tx, from_rx);
    (srv_rx, ctl_rx, from_tx)
}

fn build_harness() -> Harness {
    let mut host = App::new();
    host.add_message::<OutboundPlayerPacket>();
    host.init_resource::<DimChannelsResource>();
    host.init_resource::<DimDespawnQueue>();
    host.init_resource::<InFlightMoves>();

    let source_label = host
        .world_mut()
        .spawn((DimSubAppHandle, DimLabel(SOURCE_NAME.to_string())))
        .id();
    let dest_label = host
        .world_mut()
        .spawn((DimSubAppHandle, DimLabel(DEST_NAME.to_string())))
        .id();

    // The moving player currently lives in the source dim at epoch 0.
    let session_anchor = host
        .world_mut()
        .spawn(SessionBundle::placed(
            SESSION,
            SessionPlacement::new(Place::InDim(source_label), 0),
        ))
        .id();

    let (_src_srv_rx, source_ctl_rx, source_from_tx) = make_dim_channels(&mut host, source_label);
    let (_dest_srv_rx, dest_ctl_rx, dest_from_tx) = make_dim_channels(&mut host, dest_label);

    Harness {
        host,
        source_label,
        dest_label,
        session_anchor,
        source_from_tx,
        source_ctl_rx,
        dest_from_tx,
        dest_ctl_rx: Some(dest_ctl_rx),
    }
}

/// A standalone "source dim" world holding the in-transit entity, a faithful
/// channel→message drain, and the REAL keep-until-confirm systems.
fn build_source_dim(ctl_rx: flume::Receiver<ToDim>) -> App {
    let mut dim = App::new();
    dim.add_schedule(Schedule::new(FixedPreUpdate));
    dim.add_schedule(Schedule::new(FixedUpdate));
    dim.add_message::<InboundConfirmMove>();
    dim.add_message::<InboundRollbackMove>();
    let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    drop(srv_tx);
    dim.insert_resource(ToDimReceiver::<ToDim> {
        serverbound: srv_rx,
        control: ctl_rx,
    });
    dim.add_systems(FixedPreUpdate, drain_source_control);
    dim.add_systems(FixedUpdate, (despawn_on_confirm, unhide_on_rollback));
    dim
}

/// Mirror of the production `drain_to_dim_inbox` control arms: turn inbound
/// control messages into the sub-app messages the real systems read.
fn drain_source_control(
    rx: Res<ToDimReceiver<ToDim>>,
    mut confirm: ResMut<Messages<InboundConfirmMove>>,
    mut rollback: ResMut<Messages<InboundRollbackMove>>,
) {
    for msg in rx.control.try_iter() {
        match msg {
            ToDim::ConfirmMove(InboundConfirmMove { move_id }) => {
                confirm.write(InboundConfirmMove { move_id });
            }
            ToDim::RollbackMove(InboundRollbackMove { move_id }) => {
                rollback.write(InboundRollbackMove { move_id });
            }
            _ => {}
        }
    }
}

fn drive_dim(dim: &mut App) {
    dim.world_mut().run_schedule(FixedPreUpdate);
    dim.world_mut().run_schedule(FixedUpdate);
}

/// Spawn the source entity hidden in transit, exactly as the source does at
/// move-out (stamp `InTransit` with the move id), and emit the move request.
fn initiate_move(h: &Harness, source_dim: &mut App, move_id: MoveId) -> Entity {
    let entity = source_dim
        .world_mut()
        .spawn((
            InTransit { move_id },
            Transform::from_translation(START_POS),
        ))
        .id();
    h.source_from_tx
        .send(FromDim::MoveEntity {
            move_id,
            target: DEST_NAME.to_string(),
            cause: ArrivalCause::CommandTeleport {
                pos: DVec3::new(0.0, 100.0, 0.0),
            },
            payload: MovePayload::Player {
                uuid: Uuid::nil(),
                username: "mover".to_string(),
            },
            player: Some(SESSION),
        })
        .expect("send MoveEntity");
    entity
}

fn placement(h: &Harness) -> SessionPlacement {
    *h.host
        .world()
        .get::<SessionPlacement>(h.session_anchor)
        .expect("session")
}

fn epoch(h: &Harness) -> u32 {
    placement(h).epoch()
}

fn in_flight_present(h: &Harness, move_id: MoveId) -> bool {
    h.host
        .world()
        .resource::<InFlightMoves>()
        .get(move_id)
        .is_some()
}

#[test]
fn confirmed_move_keeps_source_until_confirm_then_despawns() {
    let mut h = build_harness();
    let mut source_dim = build_source_dim(h.source_ctl_rx.clone());
    let move_id = MoveIds::new(Entity::PLACEHOLDER).allocate();

    let entity = initiate_move(&h, &mut source_dim, move_id);

    // Host brokers the move: epoch bumps, in-flight entry created, spawn forwarded.
    pump_channels(&mut h.host);

    assert_eq!(epoch(&h), 1, "player epoch must bump on a cross-dim move");
    assert_eq!(
        placement(&h).place(),
        Place::Transferring {
            from: h.source_label,
            to: h.dest_label,
        },
        "the session must be moving to the destination"
    );
    assert!(
        in_flight_present(&h, move_id),
        "host tracks the in-flight move"
    );

    // Target received the spawn command, keyed on the source's move id and the
    // post-bump epoch.
    match h.dest_ctl_rx.as_ref().unwrap().try_recv() {
        Ok(ToDim::SpawnEntity(InboundEntitySpawn {
            move_id: forwarded,
            epoch: stamped,
            ..
        })) => {
            assert_eq!(forwarded, move_id, "spawn must carry the source's move id");
            assert_eq!(stamped, 1, "spawn must carry the post-bump epoch");
        }
        other => panic!("expected SpawnEntity, got {other:?}"),
    }

    // Keep-until-confirm: before the ack the source entity is still alive and
    // hidden — never despawned at move-out.
    assert!(
        source_dim.world().get::<InTransit>(entity).is_some(),
        "source entity stays in transit until confirm"
    );
    assert!(
        source_dim.world().get::<Despawned>(entity).is_none(),
        "source entity must NOT be despawned at move-out"
    );

    // Target acks Spawned; host relays ConfirmMove to the source.
    h.dest_from_tx
        .send(FromDim::Spawned { move_id })
        .expect("send Spawned");
    pump_channels(&mut h.host);
    assert!(
        !in_flight_present(&h, move_id),
        "in-flight entry cleared on Spawned"
    );
    assert_eq!(
        placement(&h).place(),
        Place::InDim(h.dest_label),
        "the arrival attaches the session to the destination"
    );

    // The real despawn-on-confirm system matches the entity by the echoed id and
    // despawns it — proving the id reconciliation holds end to end.
    drive_dim(&mut source_dim);
    assert!(
        source_dim.world().get::<Despawned>(entity).is_some(),
        "source entity is despawned only after confirm"
    );
    assert!(
        source_dim.world().get::<InTransit>(entity).is_none(),
        "in-transit marker cleared on confirm"
    );
}

#[test]
fn never_acked_move_rolls_back_on_tick_timeout() {
    let mut h = build_harness();
    h.host
        .world_mut()
        .resource_mut::<InFlightMoves>()
        .timeout_ticks = 3;
    let mut source_dim = build_source_dim(h.source_ctl_rx.clone());
    let move_id = MoveIds::new(Entity::PLACEHOLDER).allocate();

    let entity = initiate_move(&h, &mut source_dim, move_id);

    // The target never acks. Each host tick advances the tick counter; after the
    // threshold the host emits RollbackMove. Run a few ticks, draining the
    // source each time, until it un-hides.
    let mut rolled_back = false;
    for _ in 0..6 {
        pump_channels(&mut h.host);
        expire_moves(&mut h.host);
        drive_dim(&mut source_dim);
        if source_dim.world().get::<InTransit>(entity).is_none() {
            rolled_back = true;
            break;
        }
    }

    assert!(rolled_back, "timeout must roll the move back");
    assert_eq!(
        placement(&h).place(),
        Place::InDim(h.source_label),
        "a rolled-back move leaves the session where it was"
    );
    assert!(
        !in_flight_present(&h, move_id),
        "timed-out entry removed from the in-flight table"
    );
    // Rollback un-hides the SAME entity at its ORIGINAL position — zero loss,
    // never a despawn.
    assert!(
        source_dim.world().get::<Despawned>(entity).is_none(),
        "rollback must not despawn the source entity"
    );
    assert_eq!(
        source_dim
            .world()
            .get::<Transform>(entity)
            .expect("transform")
            .translation,
        START_POS,
        "rolled-back entity reappears where it left"
    );
}

/// The loop pumps the channels every few milliseconds while it waits for the
/// next tick, so a timeout counted in pumps would fire in a fraction of its
/// intended time.
#[test]
fn pumping_between_ticks_does_not_age_a_move() {
    let mut h = build_harness();
    h.host
        .world_mut()
        .resource_mut::<InFlightMoves>()
        .timeout_ticks = 3;
    let mut source_dim = build_source_dim(h.source_ctl_rx.clone());
    let move_id = MoveIds::new(Entity::PLACEHOLDER).allocate();

    let entity = initiate_move(&h, &mut source_dim, move_id);

    for _ in 0..10 {
        pump_channels(&mut h.host);
        drive_dim(&mut source_dim);
    }

    assert!(in_flight_present(&h, move_id), "only ticks age a move");
    assert!(
        source_dim.world().get::<InTransit>(entity).is_some(),
        "the entity stays in transit"
    );
}

#[test]
fn disconnected_target_rolls_back_immediately() {
    let mut h = build_harness();
    // Drop the target's control receiver so the host's spawn send reports
    // Disconnected on the very first pass.
    h.dest_ctl_rx = None;
    let mut source_dim = build_source_dim(h.source_ctl_rx.clone());
    let move_id = MoveIds::new(Entity::PLACEHOLDER).allocate();

    let entity = initiate_move(&h, &mut source_dim, move_id);

    pump_channels(&mut h.host);
    drive_dim(&mut source_dim);

    assert!(
        source_dim.world().get::<InTransit>(entity).is_none(),
        "a disconnected target rolls the move back at once"
    );
    assert_eq!(
        placement(&h).place(),
        Place::InDim(h.source_label),
        "the session stays in the dimension it never left"
    );
    assert!(
        source_dim.world().get::<Despawned>(entity).is_none(),
        "immediate rollback must not despawn the source entity"
    );
    assert!(
        !in_flight_present(&h, move_id),
        "no in-flight entry survives an immediate rollback"
    );
    assert_eq!(
        source_dim
            .world()
            .get::<Transform>(entity)
            .expect("transform")
            .translation,
        START_POS,
        "entity stays at its original position"
    );
}
