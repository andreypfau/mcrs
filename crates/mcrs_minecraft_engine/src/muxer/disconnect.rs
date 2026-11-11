use bevy_app::{App, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::system::{Commands, Query, ResMut};
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, Session, SessionAnchors, SessionPlacement,
};
use mcrs_minecraft_network::ServerSideConnection;
use mcrs_minecraft_protocol::uuid::Uuid;

use crate::bus::InboundPlayerDespawn;
use crate::channel_types::ToDim;
use crate::game_profile::GameProfile;
use crate::muxer::bridge_queue::OutboundQueue;
use crate::muxer::dim_inputs::DimInputs;
use crate::muxer::session::{HostAnchorRef, SessionConnection};

pub type LeavingSessions<'w, 's> = Query<
    'w,
    's,
    (
        &'static Session,
        &'static mut SessionPlacement,
        Option<&'static SessionConnection>,
        Option<&'static GameProfile>,
    ),
>;

/// A player a dimension has been told to let go of and has not yet saved. A login under its id
/// waits for it, so the new session never reads a player file the old one has still to write.
#[derive(Component, Debug)]
pub struct Departing {
    pub id: Uuid,
    pub dim: Entity,
    pub session: PlayerSession,
}

#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "disconnect::on_player_disconnect", skip_all)
)]
pub fn on_player_disconnect(
    trigger: On<Remove<ServerSideConnection>>,
    connection_refs: Query<&HostAnchorRef>,
    mut sessions: LeavingSessions,
    mut dim_channels: ResMut<DimInputs>,
    mut commands: Commands,
) {
    let Ok(host_anchor_ref) = connection_refs.get(trigger.event().entity) else {
        return;
    };
    process_disconnect(
        host_anchor_ref.0,
        &mut sessions,
        &mut dim_channels,
        &mut commands,
    );
}

/// Run a single host-anchor's cleanup: route the despawn into the dimension
/// that holds the player and despawn the host anchor, which takes the session
/// with it.
///
/// The session is unplaced before its anchor's despawn is applied, so a second
/// cleanup of the same anchor routes nothing.
pub fn process_disconnect(
    host_anchor: Entity,
    sessions: &mut LeavingSessions,
    dim_channels: &mut DimInputs,
    commands: &mut Commands,
) {
    let Ok((session, mut placement, connection, profile)) = sessions.get_mut(host_anchor) else {
        return;
    };

    let releasing = despawn_from_dim(session.0, placement.place(), dim_channels);
    if let Some(profile) = profile
        && let Some(dim) = releasing
    {
        commands.spawn(Departing {
            id: profile.id,
            dim,
            session: session.0,
        });
    }
    placement.set(Place::Unplaced);

    if let Some(connection) = connection
        && let Ok(mut socket_entity) = commands.get_entity(connection.entity())
    {
        socket_entity.try_remove::<OutboundQueue>();
    }

    if let Ok(mut anchor_entity) = commands.get_entity(host_anchor) {
        anchor_entity.try_despawn();
    }
}

/// Routes the player's despawn into its dimension. Returns the dimension when it took it.
pub fn despawn_from_dim(
    session: PlayerSession,
    place: Place,
    dim_channels: &mut DimInputs,
) -> Option<Entity> {
    place.dim().filter(|&dim| {
        dim_channels.send_control(dim, ToDim::Despawn(InboundPlayerDespawn { session }))
    })
}

pub struct DisconnectProtocolPlugin;

impl Plugin for DisconnectProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SessionAnchors>();
        app.add_observer(on_player_disconnect);
    }
}
