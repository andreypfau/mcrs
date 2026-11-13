extern crate alloc;

use bytes::Bytes;
use core::net::SocketAddr;
use std::io::ErrorKind;
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

use crate::reliability::Reliability;
use crate::server::{RaknetEvent, RaknetServer};
use crate::unconnected::Motd;

pub const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverError {
    ChannelClosed,
    InvalidChannel,
}

enum DriverCommand {
    Send {
        peer: SocketAddr,
        payload: Bytes,
        reliability: Reliability,
        channel: u8,
    },
    Disconnect {
        peer: SocketAddr,
    },
    Close,
}

pub struct RaknetListener {
    cmd_tx: mpsc::Sender<DriverCommand>,
    event_rx: mpsc::Receiver<RaknetEvent>,
    worker: tokio::task::JoinHandle<()>,
}

impl RaknetListener {
    pub async fn bind(addr: SocketAddr, server_guid: u64, motd: Motd) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        let server = RaknetServer::new(server_guid, motd);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
        let (event_tx, event_rx) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);

        let worker = tokio::spawn(async move {
            let mut server = server;
            let mut rx_buf = [0u8; 4096];
            let mut tx_buf = [0u8; 4096];
            let start = Instant::now();
            let mut tick_timer = interval(Duration::from_millis(10));

            loop {
                tokio::select! {
                    cmd_opt = cmd_rx.recv() => {
                        let Some(cmd) = cmd_opt else {
                            break;
                        };
                        match cmd {
                            DriverCommand::Send { peer, payload, reliability, channel } => {
                                let _ = server.send(peer, payload, reliability, channel);
                            }
                            DriverCommand::Disconnect { peer } => {
                                server.disconnect(peer);
                            }
                            DriverCommand::Close => {
                                break;
                            }
                        }
                    }
                    recv_res = socket.recv_from(&mut rx_buf) => {
                        match recv_res {
                            Ok((size, peer_addr)) => {
                                let now_ms = start.elapsed().as_millis() as u64;
                                if let Some(slice) = rx_buf.get(..size) {
                                    server.accept_inbound(peer_addr, slice, now_ms);
                                }
                            }
                            Err(e) => {
                                if e.kind() == ErrorKind::Interrupted
                                    || e.kind() == ErrorKind::ConnectionReset
                                    || e.kind() == ErrorKind::WouldBlock
                                {
                                    continue;
                                }
                                break;
                            }
                        }
                    }
                    _ = tick_timer.tick() => {
                        let now_ms = start.elapsed().as_millis() as u64;
                        server.step(now_ms);
                    }
                }

                while let Some(event) = server.poll_event() {
                    if event_tx.send(event).await.is_err() {
                        return;
                    }
                }

                while let Some((len, target)) = server.poll_outbound_slice(&mut tx_buf) {
                    if let Some(slice) = tx_buf.get(..len) {
                        let _ = socket.send_to(slice, target).await;
                    }
                }
            }
        });

        Ok(Self {
            cmd_tx,
            event_rx,
            worker,
        })
    }

    pub async fn recv(&mut self) -> Option<RaknetEvent> {
        self.event_rx.recv().await
    }

    pub async fn send(&self, peer: SocketAddr, payload: Bytes) -> Result<(), DriverError> {
        self.cmd_tx
            .send(DriverCommand::Send {
                peer,
                payload,
                reliability: Reliability::ReliableOrdered,
                channel: 0,
            })
            .await
            .map_err(|_| DriverError::ChannelClosed)
    }

    pub async fn send_with(
        &self,
        peer: SocketAddr,
        payload: Bytes,
        reliability: Reliability,
        channel: u8,
    ) -> Result<(), DriverError> {
        if (channel as usize) >= crate::session::MAX_ORDER_CHANNELS {
            return Err(DriverError::InvalidChannel);
        }
        self.cmd_tx
            .send(DriverCommand::Send {
                peer,
                payload,
                reliability,
                channel,
            })
            .await
            .map_err(|_| DriverError::ChannelClosed)
    }

    pub async fn disconnect(&self, peer: SocketAddr) -> Result<(), DriverError> {
        self.cmd_tx
            .send(DriverCommand::Disconnect { peer })
            .await
            .map_err(|_| DriverError::ChannelClosed)
    }

    pub async fn close(self) {
        let _ = self.cmd_tx.send(DriverCommand::Close).await;
        self.worker.abort();
    }
}

impl Drop for RaknetListener {
    fn drop(&mut self) {
        self.worker.abort();
    }
}
