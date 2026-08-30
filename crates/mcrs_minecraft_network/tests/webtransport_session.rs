use bevy_app::{App, FixedPreUpdate};
use mcrs_minecraft_network::packet_io::PacketIo;
use mcrs_minecraft_network::{NetworkPlugin, ServerSideConnection, WebTransportEndpoint};
use mcrs_minecraft_protocol::handshake::Intent;
use mcrs_minecraft_protocol::packets::intent::serverbound::ServerboundHandshake;
use mcrs_minecraft_protocol::packets::status::clientbound::StatusResponse;
use mcrs_minecraft_protocol::packets::status::serverbound::StatusRequest;
use mcrs_minecraft_protocol::{Bounded, PROTOCOL_VERSION, VarInt};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;
use wtransport::endpoint::endpoint_side::Client;
use wtransport::stream::BiStream;
use wtransport::tls::Sha256Digest;
use wtransport::{ClientConfig, Connection, Endpoint};

/// The endpoint and the session handle are dropped together with the stream:
/// releasing either of them tears the QUIC connection down under the server.
struct WebTransportPeer {
    _endpoint: Endpoint<Client>,
    _session: Connection,
    io: PacketIo<BiStream>,
}

async fn open_session(
    server: WebTransportEndpoint,
    intent: Intent,
) -> anyhow::Result<WebTransportPeer> {
    let config = ClientConfig::builder()
        .with_bind_default()
        .with_server_certificate_hashes([Sha256Digest::new(server.certificate_hash)])
        .build();
    let endpoint = Endpoint::client(config)?;
    let session = endpoint
        .connect(format!("https://127.0.0.1:{}/", server.address.port()))
        .await?;
    let (send, recv) = session.open_bi().await?.await?;
    let mut io = PacketIo::new(BiStream::join((send, recv)));

    io.send_packet(&ServerboundHandshake {
        protocol_version: VarInt(PROTOCOL_VERSION),
        server_address: Bounded("127.0.0.1"),
        server_port: server.address.port(),
        intent,
    })
    .await?;

    Ok(WebTransportPeer {
        _endpoint: endpoint,
        _session: session,
        io,
    })
}

#[test]
fn a_browser_transport_session_drives_a_status_exchange_and_a_login() {
    let mut app = App::new();
    app.add_plugins(NetworkPlugin {
        address: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into(),
    });

    let server = *app.world().resource::<WebTransportEndpoint>();
    assert_ne!(server.address.port(), 0, "the UDP port was not reported");
    assert_eq!(
        server.certificate_hash_hex().len(),
        64,
        "the certificate hash is not a SHA-256 digest"
    );

    app.update();

    let client_runtime = tokio::runtime::Runtime::new().expect("a runtime for the test peer");

    let json = client_runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), async {
            let mut peer = open_session(server, Intent::Status).await?;
            peer.io.send_packet(&StatusRequest).await?;
            let response = peer.io.recv_packet::<StatusResponse>().await?;
            anyhow::Ok(response.json.to_owned())
        })
        .await
        .expect("the status exchange timed out")
        .expect("the status exchange failed")
    });
    assert!(
        json.contains(&PROTOCOL_VERSION.to_string()),
        "the status response did not carry the protocol version: {json}"
    );

    let _login = client_runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(20), open_session(server, Intent::Login))
            .await
            .expect("the login session timed out")
            .expect("the login session failed")
    });

    let mut connections = app.world_mut().query::<&ServerSideConnection>();
    for _ in 0..200 {
        app.world_mut().run_schedule(FixedPreUpdate);
        if connections.iter(app.world()).count() == 1 {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("the login session never reached the ECS as a ServerSideConnection");
}
