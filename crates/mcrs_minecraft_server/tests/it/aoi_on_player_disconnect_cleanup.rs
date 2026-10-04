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

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft_level::session::{Place, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::channels::{
    FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_minecraft_server::disconnect::{
    DisconnectProtocolPlugin, Disconnects, filter_inflight_for_disconnect,
};
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
};
use mcrs_minecraft_server::world::channel_types::FromDim;
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_server::world::session::SessionBundle;

fn build_disconnect_app() -> App {
    let mut app = App::new();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.add_plugins(DisconnectProtocolPlugin);
    app
}

/// Register a dim channel in the app's `DimChannelsResource` and return the
/// control receiver so tests can assert on `ToDim::Despawn` messages.
fn register_dim_channel(app: &mut App, dim: Entity) -> flume::Receiver<ToDim> {
    let (srv_tx, _srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(dim, srv_tx, ctl_tx, from_rx);
    ctl_rx
}

fn count_despawns(rx: &flume::Receiver<ToDim>) -> usize {
    rx.try_iter()
        .filter(|m| matches!(m, ToDim::Despawn(..)))
        .count()
}

fn insert_location(
    app: &mut App,
    host_anchor: Entity,
    current_dim: Entity,
    previous_dim: Option<Entity>,
    in_dim_entity: Option<Entity>,
) {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    let place = match (previous_dim, in_dim_entity) {
        (Some(from), _) => Place::Transferring {
            from,
            to: current_dim,
        },
        (None, Some(_)) => Place::InDim(current_dim),
        (None, None) => Place::Joining(current_dim),
    };
    app.world_mut()
        .entity_mut(host_anchor)
        .insert(SessionBundle::placed(
            session,
            SessionPlacement::new(place, 0),
        ));
}

fn synthetic_disconnect(app: &mut App, host_anchor: Entity) {
    app.world_mut()
        .run_system_once(move |mut disconnects: Disconnects| disconnects.disconnect(host_anchor))
        .expect("disconnect helper runs");
}

fn run_filter(app: &mut App) {
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("filter runs");
}

#[test]
fn disconnect_at_tick_n_e1_4_attached_pending_filter() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let source_dim = Entity::from_raw_u32(401).unwrap();
    let dest_dim = Entity::from_raw_u32(402).unwrap();
    let src_ctl_rx = register_dim_channel(&mut app, source_dim);
    let dst_ctl_rx = register_dim_channel(&mut app, dest_dim);
    insert_location(&mut app, host_anchor, dest_dim, Some(source_dim), None);

    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>()
        .write(OutboundPlayerAttached { host_anchor });

    synthetic_disconnect(&mut app, host_anchor);
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

    assert_eq!(count_despawns(&dst_ctl_rx), 1);
    assert_eq!(count_despawns(&src_ctl_rx), 1);

    assert!(
        app.world().get_entity(host_anchor).is_err(),
        "the session is despawned"
    );
}
