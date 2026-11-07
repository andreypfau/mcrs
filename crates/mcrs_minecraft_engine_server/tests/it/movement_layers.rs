use std::time::Duration;

use mcrs_minecraft_engine_server::{InboundRateLimit, ServerNetworkPlugin};
use mcrs_minecraft_network::RawConnection;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundDisconnect;
use mcrs_minecraft_protocol::{Decode, Encode, Packet};
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundClientTickEnd, ServerboundMovePlayerPos,
};
use mcrs_minecraft_protocol_game::{MoveFlags, Position};

use crate::server::RunningServer;

/// Sends `packet` every `interval` until the server disconnects, and answers the reason.
async fn send_until_disconnected<P: Encode + Packet>(
    connection: &mut RawConnection,
    packet: &P,
    interval: Duration,
) -> String {
    loop {
        connection.append(packet).unwrap();
        connection.flush().unwrap();
        while let Some(received) = connection.try_recv().unwrap() {
            if received.id == ClientboundDisconnect::ID {
                let disconnect = ClientboundDisconnect::decode(&mut &received.payload[..]).unwrap();
                return format!("{:?}", disconnect.reason);
            }
        }
        tokio::time::sleep(interval).await;
    }
}

fn reason_of<P: Encode + Packet>(
    network: ServerNetworkPlugin,
    packet: &P,
    interval: Duration,
) -> String {
    let server = RunningServer::start(network);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let reason = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(60), async {
            let mut connection = server.join().await;
            send_until_disconnected(&mut connection, packet, interval).await
        })
        .await
        .expect("a disconnect within the timeout")
    });
    server.stop();
    reason
}

/// MVC-CHK-023, MVC-LAY-002: the reader of a real connection runs layer 0 in the play state.
#[test]
fn a_move_with_a_nan_coordinate_disconnects_a_joined_client() {
    let not_finite = ServerboundMovePlayerPos {
        position: Position::new(f64::NAN, 64.0, 0.0),
        flags: MoveFlags::new(),
    };
    let reason = reason_of(
        ServerNetworkPlugin::embedded(),
        &not_finite,
        Duration::from_millis(50),
    );
    assert!(
        reason.contains("multiplayer.disconnect.invalid_player_movement"),
        "{reason}"
    );
}

/// MVC-VAN-019: with the `rate-limit` property set, a client that keeps sending 100 packets each
/// second is disconnected.
#[test]
fn a_rate_above_the_limit_disconnects_a_joined_client() {
    let network = ServerNetworkPlugin {
        rate_limit: InboundRateLimit(50),
        ..ServerNetworkPlugin::embedded()
    };
    let reason = reason_of(
        network,
        &ServerboundClientTickEnd,
        Duration::from_millis(10),
    );
    assert!(
        reason.contains("disconnect.exceeded_packet_rate"),
        "{reason}"
    );
}
