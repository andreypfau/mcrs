use bevy::app::{App, Plugin, Update};
use bevy::ecs::prelude::{
    Commands, Component, IntoScheduleConfigs, On, Query, Res, Resource, Single,
};
use bevy::math::DVec3;
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

/// Keeps on the connection where the server puts the player and which columns it keeps
/// loaded around it, and tells the server the view distance that the options ask for.
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
                    .run_if(view_distance_unsent),
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

/// Teleports the server has sent that the player has yet to be moved to and
/// confirm. A queue rather than the latest one, because the server keeps a
/// count of outstanding confirmations and a dropped teleport never unblocks.
#[derive(Component, Default, Debug)]
pub struct PendingTeleports(pub Vec<ServerTeleport>);

/// The column the server centres the client's loaded area on.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheCenter(pub ColumnPos);

/// The radius in columns of the area the server keeps loaded for the client.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkCacheRadius(pub i32);

/// The render distance in columns that the options of the user ask for.
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
    requested: Res<RequestedViewDistance>,
    connection: Single<(&mut ClientConnection, &mut SentViewDistance)>,
) {
    let (mut connection, mut sent) = connection.into_inner();
    connection.write_packet(&ServerboundClientInformation(join::client_information(
        requested.0,
    )));
    sent.0 = requested.0;
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
    use bevy::ecs::prelude::{Entity, With};
    use bytes::BytesMut;
    use mcrs_minecraft_network::client::{ClientNetworkPlugin, finish_configuration};
    use mcrs_minecraft_protocol::decode::{PacketDecoder, PacketFrame};
    use mcrs_minecraft_protocol::packets::common::serverbound::ClientInformation;
    use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundFinishConfiguration;
    use mcrs_minecraft_protocol::packets::configuration::serverbound::{
        ServerboundClientInformation as ConfigurationInformation, ServerboundFinishConfiguration,
    };
    use mcrs_minecraft_protocol::packets::login::clientbound::{
        ClientboundLoginFinishedWithOptions, OptionFlags,
    };
    use mcrs_minecraft_protocol::profile::GameProfile;
    use mcrs_minecraft_protocol::uuid::Uuid;
    use mcrs_minecraft_protocol::{Bounded, Decode, Encode, Packet, PacketEncoder};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    struct Server {
        stream: TcpStream,
        decoder: PacketDecoder,
    }

    impl Server {
        /// Answers the login and ends the configuration. Returns the `client_information` of the
        /// configuration state.
        fn join(listener: &TcpListener) -> (Server, PacketFrame) {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut server = Server {
                stream,
                decoder: PacketDecoder::new(),
            };
            let _handshake = server.next_frame();
            let _hello = server.next_frame();
            server.send(&ClientboundLoginFinishedWithOptions {
                profile: GameProfile {
                    id: Uuid::nil(),
                    username: Bounded("Steve"),
                    properties: Default::default(),
                },
                session_id: Uuid::nil(),
                options: OptionFlags::new(),
            });
            let _acknowledged = server.next_frame();
            let _brand = server.next_frame();
            let _mod_list = server.next_frame();
            let information = server.next_frame();
            assert_eq!(
                information.id,
                ConfigurationInformation::ID,
                "the configuration starts with the client information"
            );
            server.send(&ClientboundFinishConfiguration);
            (server, information)
        }

        fn next_frame(&mut self) -> PacketFrame {
            loop {
                if let Some(frame) = self.decoder.try_next_packet().unwrap() {
                    return frame;
                }
                let mut buffer = [0; 1024];
                let read = self.stream.read(&mut buffer).unwrap();
                assert_ne!(read, 0, "the client closed the connection");
                self.decoder.queue_bytes(BytesMut::from(&buffer[..read]));
            }
        }

        fn send(&mut self, packet: &(impl Encode + Packet)) {
            let mut encoder = PacketEncoder::new();
            encoder.write_packet(packet);
            self.stream.write_all(&encoder.take()).unwrap();
        }

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
                server: listener.local_addr().unwrap(),
                username: "Steve".to_owned(),
                profile_id: None,
                view_distance,
                holds_vanilla_pack: false,
                play: None,
            },
            PlayerViewPlugin,
        ))
        .insert_resource(RequestedViewDistance(view_distance));
        app
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

    fn request(app: &mut App, view_distance: u8) {
        app.world_mut().resource_mut::<RequestedViewDistance>().0 = view_distance;
        app.update();
    }

    #[test]
    fn the_play_state_sends_a_view_distance_that_differs_from_the_last_sent() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut app = client(&listener, 8);
        let (mut server, configuration) = Server::join(&listener);
        let configured = ClientInformation::decode(&mut &configuration.body[..]).unwrap();
        let connection = spawned_connection(&mut app);

        request(&mut app, 12);
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

        request(&mut app, 12);
        request(&mut app, 16);
        server.assert_next_information(configured, 16);
        request(&mut app, 8);
        server.assert_next_information(configured, 8);
    }
}
