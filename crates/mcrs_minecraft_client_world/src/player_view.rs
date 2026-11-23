use bevy::app::{App, Plugin, Update};
use bevy::ecs::prelude::{
    Commands, Component, DetectChanges, IntoScheduleConfigs, Local, On, Query, Res, Resource,
    SystemCondition, resource_exists, resource_exists_and_changed,
};
use bevy::math::DVec3;
use bevy::time::{Real, Time};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems, SentViewDistance};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_network::join;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundChunkCacheRadius, ClientboundLogin, ClientboundPlayerPosition,
    ClientboundSetChunkCacheCenter,
};
use mcrs_minecraft_protocol_game::serverbound::ServerboundClientInformation;
use mcrs_minecraft_protocol_game::{ColumnPos, PositionFlags};
use std::time::Duration;

const SMALLER_VIEW_DISTANCE_WAIT: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerViewPlugin;

impl Plugin for PlayerViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_player_position)
            .add_observer(on_set_chunk_cache_center)
            .add_observer(on_chunk_cache_radius)
            .add_observer(on_login)
            .add_systems(
                Update,
                send_view_distance
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush)
                    .run_if(
                        resource_exists::<Time<Real>>.and_then(
                            resource_exists_and_changed::<RequestedViewDistance>
                                .or_else(view_distance_unsent),
                        ),
                    ),
            );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerTeleport {
    pub teleport_id: i32,
    pub position: DVec3,
    pub velocity: DVec3,
    pub look: Look,
    pub flags: PositionFlags,
}

#[derive(Component, Default, Debug)]
pub struct PendingTeleports(pub Vec<ServerTeleport>);

#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheCenter(pub ColumnPos);

#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheRadius(pub i32);

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestedViewDistance(pub u8);

fn view_distance_unsent(
    requested: Option<Res<RequestedViewDistance>>,
    connections: Query<(&SentViewDistance, &ConnectionState)>,
) -> bool {
    let (Some(requested), Ok((sent, state))) = (requested, connections.single()) else {
        return false;
    };
    *state == ConnectionState::Game && sent.0 != requested.0
}

fn send_view_distance(
    time: Res<Time<Real>>,
    requested: Res<RequestedViewDistance>,
    mut changed_at: Local<Duration>,
    mut connections: Query<(
        &mut ClientConnection,
        &mut SentViewDistance,
        &ConnectionState,
    )>,
) {
    let now = time.elapsed();
    if requested.is_changed() {
        *changed_at = now;
    }
    let Ok((mut connection, mut sent, state)) = connections.single_mut() else {
        return;
    };
    if *state != ConnectionState::Game || requested.0 == sent.0 {
        return;
    }
    if requested.0 < sent.0 && now.saturating_sub(*changed_at) < SMALLER_VIEW_DISTANCE_WAIT {
        return;
    }
    connection.write_packet(&ServerboundClientInformation(join::client_information(
        requested.0,
    )));
    if sent.0 != requested.0 {
        sent.0 = requested.0;
    }
}

fn on_player_position(event: On<Received<ClientboundPlayerPosition>>, mut commands: Commands) {
    let Some(position) = event.packet() else {
        return;
    };
    let teleport = ServerTeleport {
        teleport_id: position.teleport_id.0,
        position: position.position,
        velocity: position.velocity,
        look: position.look,
        flags: position.flags,
    };
    commands
        .entity(event.entity)
        .entry::<PendingTeleports>()
        .or_default()
        .and_modify(move |mut pending| pending.0.push(teleport));
}

fn on_set_chunk_cache_center(
    event: On<Received<ClientboundSetChunkCacheCenter>>,
    mut commands: Commands,
) {
    let Some(center) = event.packet() else {
        return;
    };
    commands
        .entity(event.entity)
        .insert(ChunkCacheCenter(ColumnPos::new(center.x.0, center.z.0)));
}

fn on_chunk_cache_radius(event: On<Received<ClientboundChunkCacheRadius>>, mut commands: Commands) {
    let Some(radius) = event.packet() else {
        return;
    };
    commands
        .entity(event.entity)
        .insert(ChunkCacheRadius(radius.radius.0));
}

fn on_login(event: On<Received<ClientboundLogin>>, mut commands: Commands) {
    let Some(login) = event.packet() else {
        return;
    };
    commands
        .entity(event.entity)
        .insert(ChunkCacheRadius(login.chunk_radius.0));
}

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
mod tests {
    use super::*;
    use crate::test_session::Server;
    use bevy::ecs::message::Messages;
    use bevy::ecs::prelude::{DetectChangesMut, Entity, With};
    use mcrs_minecraft_network::client::{
        ClientNetworkPlugin, ClientSession, ConnectionEnd, ConnectionEnded, JoinRequest,
        LeaveRequest, finish_configuration,
    };
    use mcrs_minecraft_protocol::packets::common::serverbound::ClientInformation;
    use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
    use mcrs_minecraft_protocol::{Decode, Packet};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    impl Server {
        fn assert_next_information(&mut self, configured: ClientInformation, view_distance: u8) {
            let frame = self.next_frame();
            assert_eq!(
                frame.id,
                ServerboundClientInformation::ID,
                "the play client information"
            );
            assert_eq!(
                ClientInformation::decode(&mut &frame.body[..]).unwrap(),
                ClientInformation {
                    view_distance,
                    ..configured
                },
                "only the view distance differs from the configuration"
            );
        }
    }

    fn client(listener: &TcpListener, view_distance: u8) -> App {
        let mut app = App::new();
        app.add_plugins((
            ClientNetworkPlugin {
                holds_vanilla_pack: false,
                play: None,
            },
            PlayerViewPlugin,
        ))
        .insert_resource(RequestedViewDistance(view_distance))
        .init_resource::<Time<Real>>();
        join(&mut app, listener, view_distance);
        app
    }

    fn join(app: &mut App, listener: &TcpListener, view_distance: u8) {
        app.world_mut().write_message(JoinRequest {
            server: listener.local_addr().unwrap(),
            username: "Steve".to_owned(),
            profile_id: None,
            view_distance,
        });
        app.update();
    }

    fn spawned_connection(app: &mut App) -> Entity {
        for _ in 0..1000 {
            app.update();
            let mut connections = app
                .world_mut()
                .query_filtered::<Entity, With<ClientConnection>>();
            if let Ok(connection) = connections.single(app.world()) {
                return connection;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("the client never joined");
    }

    fn frame(app: &mut App, elapsed_ms: u64, view_distance: u8) {
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .update_with_duration(Duration::from_millis(elapsed_ms));
        app.world_mut()
            .resource_mut::<RequestedViewDistance>()
            .set_if_neq(RequestedViewDistance(view_distance));
        app.update();
    }

    #[test]
    fn a_larger_view_distance_goes_at_once_and_a_smaller_one_after_600_ms() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut app = client(&listener, 8);
        let (mut server, configuration) = Server::join(&listener);
        let configured = ClientInformation::decode(&mut &configuration.body[..]).unwrap();
        let connection = spawned_connection(&mut app);

        frame(&mut app, 0, 12);
        let world = app.world_mut();
        finish_configuration(&mut world.commands(), connection);
        world.flush();
        app.update();
        assert_eq!(
            server.next_frame().id,
            ServerboundFinishConfiguration::ID,
            "a change in the configuration state sends nothing before the play state"
        );
        server.assert_next_information(configured, 12);

        let frames: [(u64, u8, Option<u8>); 20] = [
            (16, 13, Some(13)),
            (16, 14, Some(14)),
            (16, 15, Some(15)),
            (16, 16, Some(16)),
            (16, 15, None),
            (16, 12, None),
            (16, 10, None),
            (599, 10, None),
            (1, 10, Some(10)),
            (16, 8, None),
            (500, 9, None),
            (500, 9, None),
            (100, 9, Some(9)),
            (16, 7, None),
            (16, 11, Some(11)),
            (700, 11, None),
            (16, 8, None),
            (16, 11, None),
            (700, 11, None),
            (16, 12, Some(12)),
        ];
        for (elapsed_ms, view_distance, sent) in frames {
            frame(&mut app, elapsed_ms, view_distance);
            if let Some(sent) = sent {
                server.assert_next_information(configured, sent);
            }
        }
    }

    #[test]
    fn a_leave_ends_the_session_and_a_second_join_connects_again() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut app = client(&listener, 8);
        let _first = Server::join(&listener);
        spawned_connection(&mut app);

        app.world_mut().write_message(LeaveRequest);
        app.update();
        let ended: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<ConnectionEnded>>()
            .drain()
            .map(|ended| ended.reason)
            .collect();
        assert!(matches!(ended[..], [ConnectionEnd::Left]), "{ended:?}");
        let mut sessions = app.world_mut().query::<&ClientSession>();
        assert_eq!(sessions.iter(app.world()).count(), 0);

        join(&mut app, &listener, 8);
        let _second = Server::join(&listener);
        spawned_connection(&mut app);
    }
}
