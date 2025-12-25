use crate::SharedNetworkState;
use crate::packet_io::PacketIo;

use crate::intent::handle_intent;
use anyhow::bail;
use bevy_ecs::component::Component;
use bytes::{Bytes, BytesMut};
use log::info;
use mcrs_protocol::{Decode, Encode, Text};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const HANDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn start_accept_loop(shared: SharedNetworkState) {
    let listener = match TcpListener::bind(shared.0.address).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("Failed to bind to address {} {}", shared.0.address, e);
            return;
        }
    };
    info!("Listening on {}", shared.0.address);

    loop {
        match listener.accept().await {
            Ok((socket, remote_addr)) => {
                let shared = shared.clone();
                tokio::spawn(async move {
                    if let Err(e) = timeout(
                        HANDLE_CONNECTION_TIMEOUT,
                        handle_connection(shared, socket, remote_addr),
                    )
                    .await
                    {
                        eprintln!("{} Failed to handle connection: {}", remote_addr, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection(
    shared: SharedNetworkState,
    stream: tokio::net::TcpStream,
    remote_addr: std::net::SocketAddr,
) {
    if let Err(e) = stream.set_nodelay(true) {
        eprintln!("Failed to set nodelay on {}: {}", remote_addr, e);
    }
    let io = PacketIo::new(stream);
    if let Err(e) = handle_intent(shared, io, remote_addr).await {
        eprintln!("Error during handshake with {}: {}", remote_addr, e);
    }
}

// #[derive(Component)]
// pub struct Connection {
//     remote_addr: std::net::SocketAddr,
//     state: mcrs_protocol::PacketState,
//     recv: Receiver<ReceivedPacket>,
//     reader_task: JoinHandle<()>,
//     pub disconnect_reason: Option<Text>,
// }
//
// impl Connection {
//     pub fn remote_addr(&self) -> std::net::SocketAddr {
//         self.remote_addr
//     }
//
//     pub fn state(&self) -> mcrs_protocol::PacketState {
//         self.state
//     }
//
//     // pub async fn send_packet<P>(&mut self, pkt: &P) -> anyhow::Result<()>
//     // where
//     //     P: Encode,
//     // {
//     //     self.io.send_packet(pkt).await
//     // }
//     //
//     // pub async fn recv_packet<'a, P>(&'a mut self) -> anyhow::Result<P>
//     // where
//     //     P: Decode<'a>,
//     // {
//     //     self.io.recv_packet().await
//     // }
//
//     pub fn try_recv(&mut self) -> anyhow::Result<Option<ReceivedPacket>> {
//         match self.recv.try_recv() {
//             Ok(p) => Ok(Some(p)),
//             Err(TryRecvError::Empty) => Ok(None),
//             Err(TryRecvError::Disconnected) => bail!("client disconnected"),
//         }
//     }
// }
