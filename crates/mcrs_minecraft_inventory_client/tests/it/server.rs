use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bytes::BytesMut;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_network::client::ClientNetworkPlugin;
use mcrs_minecraft_network::client::JoinRequest;
use mcrs_minecraft_network::client::finish_configuration;
use mcrs_minecraft_protocol::Bounded;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::Encode;
use mcrs_minecraft_protocol::Packet;
use mcrs_minecraft_protocol::PacketEncoder;
use mcrs_minecraft_protocol::WritePacket;
use mcrs_minecraft_protocol::decode::PacketDecoder;
use mcrs_minecraft_protocol::decode::PacketFrame;
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundFinishConfiguration;
use mcrs_minecraft_protocol::packets::configuration::serverbound::ServerboundClientInformation;
use mcrs_minecraft_protocol::packets::login::clientbound::ClientboundLoginFinishedWithOptions;
use mcrs_minecraft_protocol::packets::login::clientbound::OptionFlags;
use mcrs_minecraft_protocol::profile::GameProfile;
use mcrs_minecraft_protocol::uuid::Uuid;

pub(crate) struct Server {
    stream: TcpStream,
    decoder: PacketDecoder,
}

impl Server {
    pub(crate) fn join(app: &mut App) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a local port binds");
        app.add_plugins(ClientNetworkPlugin {
            holds_vanilla_pack: false,
            play: None,
        });
        app.world_mut().write_message(JoinRequest {
            server: listener.local_addr().expect("the listener has an address"),
            username: "Steve".to_owned(),
            profile_id: None,
            view_distance: 8,
        });
        app.update();
        let (stream, _) = listener.accept().expect("the client connects");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("the timeout applies");
        let mut server = Self {
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
        while server.next_frame().id != ServerboundClientInformation::ID {}
        server.send(&ClientboundFinishConfiguration);
        let connection = (0..1000)
            .find_map(|_| {
                app.update();
                thread::sleep(Duration::from_millis(5));
                app.world_mut()
                    .query_filtered::<Entity, With<ClientConnection>>()
                    .single(app.world())
                    .ok()
            })
            .expect("the client joins");
        finish_configuration(app.world_mut().commands(), connection);
        app.world_mut().flush();
        app.update();
        server
    }

    pub(crate) fn next<P: Packet + for<'a> Decode<'a>>(&mut self) -> P {
        loop {
            let frame = self.next_frame();
            if frame.id == P::ID {
                return P::decode(&mut &frame.body[..]).expect("the packet decodes");
            }
        }
    }

    pub(crate) fn each_before<P: Packet + for<'a> Decode<'a>, Marker: Packet>(&mut self) -> Vec<P> {
        let mut packets = Vec::new();
        loop {
            let frame = self.next_frame();
            if frame.id == Marker::ID {
                return packets;
            }
            if frame.id == P::ID {
                packets.push(P::decode(&mut &frame.body[..]).expect("the packet decodes"));
            }
        }
    }

    fn next_frame(&mut self) -> PacketFrame {
        loop {
            if let Some(frame) = self.decoder.try_next_packet().expect("a frame decodes") {
                return frame;
            }
            let mut buffer = [0; 1024];
            let read = self.stream.read(&mut buffer).expect("the client sends");
            assert_ne!(read, 0, "the client keeps the connection");
            self.decoder.queue_bytes(BytesMut::from(
                buffer.get(..read).expect("the read fits the buffer"),
            ));
        }
    }

    fn send(&mut self, packet: &(impl Encode + Packet)) {
        let mut encoder = PacketEncoder::new();
        encoder.write_packet(packet);
        self.stream
            .write_all(&encoder.take())
            .expect("the server sends");
    }
}
