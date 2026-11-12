extern crate alloc;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use bytes::Bytes;
use core::net::SocketAddr;

use crate::reliability::Reliability;
use crate::session::{RaknetSession, SessionError, SessionState};
use crate::unconnected::{
    Motd, OpenConnectionReply1, OpenConnectionReply2, OpenConnectionRequest1,
    OpenConnectionRequest2, UnconnectedPing, UnconnectedPong, id,
};

pub const MAX_SESSIONS: usize = 1024;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaknetEvent {
    Connected {
        peer: SocketAddr,
        client_guid: u64,
        mtu: u16,
    },
    Packet {
        peer: SocketAddr,
        payload: Bytes,
    },
    Disconnected {
        peer: SocketAddr,
    },
}

pub type TransportEvent = RaknetEvent;

pub struct RaknetServer {
    server_guid: u64,
    motd: Motd,
    sessions: BTreeMap<SocketAddr, RaknetSession>,
    events: VecDeque<RaknetEvent>,
    outbound: VecDeque<(SocketAddr, Bytes)>,
}

impl RaknetServer {
    #[must_use]
    pub const fn new(server_guid: u64, motd: Motd) -> Self {
        Self {
            server_guid,
            motd,
            sessions: BTreeMap::new(),
            events: VecDeque::new(),
            outbound: VecDeque::new(),
        }
    }

    #[must_use]
    pub const fn server_guid(&self) -> u64 {
        self.server_guid
    }

    #[must_use]
    pub fn motd(&self) -> &Motd {
        &self.motd
    }

    pub fn set_motd(&mut self, motd: Motd) {
        self.motd = motd;
    }

    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    #[must_use]
    pub fn has_session(&self, peer: &SocketAddr) -> bool {
        self.sessions.contains_key(peer)
    }

    fn handle_unconnected(
        &mut self,
        peer_addr: SocketAddr,
        packet_id: u8,
        buf: &[u8],
        now_ms: u64,
    ) -> bool {
        match packet_id {
            id::UNCONNECTED_PING | id::UNCONNECTED_PING_OPEN_CONNECTIONS => {
                if let Some(ping) = UnconnectedPing::read_from(buf) {
                    let pong = UnconnectedPong {
                        client_time: ping.client_time,
                        server_guid: self.server_guid,
                        motd: self.motd.to_string_lossless(),
                    };
                    let mut resp_buf = [0u8; 2048];
                    if let Some(slice) = pong
                        .write_to(&mut resp_buf)
                        .and_then(|len| resp_buf.get(..len))
                    {
                        self.outbound
                            .push_back((peer_addr, Bytes::copy_from_slice(slice)));
                    }
                }
                true
            }
            id::OPEN_CONNECTION_REQUEST_1 => {
                if let Some(req) = OpenConnectionRequest1::read_from(buf) {
                    let clamped_mtu = req.mtu_size.clamp(576, 1492);
                    let reply = OpenConnectionReply1 {
                        server_guid: self.server_guid,
                        security: false,
                        mtu_size: clamped_mtu,
                    };
                    let mut resp_buf = [0u8; 128];
                    if let Some(slice) = reply
                        .write_to(&mut resp_buf)
                        .and_then(|len| resp_buf.get(..len))
                    {
                        self.outbound
                            .push_back((peer_addr, Bytes::copy_from_slice(slice)));
                    }
                }
                true
            }
            id::OPEN_CONNECTION_REQUEST_2 => {
                self.handle_ocr2(peer_addr, buf, now_ms);
                true
            }
            _ => false,
        }
    }

    fn handle_ocr2(&mut self, peer_addr: SocketAddr, buf: &[u8], now_ms: u64) {
        let Some(req) = OpenConnectionRequest2::read_from(buf) else {
            return;
        };
        let is_active = self
            .sessions
            .get(&peer_addr)
            .is_some_and(|s| s.state() == SessionState::Active);
        if is_active {
            return;
        }

        if self.sessions.len() >= MAX_SESSIONS && !self.sessions.contains_key(&peer_addr) {
            return;
        }

        let clamped_mtu = req.mtu_size.clamp(576, 1492);
        let reply = OpenConnectionReply2 {
            server_guid: self.server_guid,
            client_address: peer_addr,
            mtu_size: clamped_mtu,
            security: false,
        };
        let mut resp_buf = [0u8; 128];
        if let Some(slice) = reply
            .write_to(&mut resp_buf)
            .and_then(|len| resp_buf.get(..len))
        {
            self.outbound
                .push_back((peer_addr, Bytes::copy_from_slice(slice)));
        }

        if let Ok(mut session) =
            RaknetSession::new(peer_addr, clamped_mtu, self.server_guid, now_ms)
        {
            session.set_client_guid(req.client_guid);
            self.sessions.insert(peer_addr, session);
        }
    }

    pub fn accept_inbound(&mut self, peer_addr: SocketAddr, buf: &[u8], now_ms: u64) {
        let Some(&packet_id) = buf.first() else {
            return;
        };

        if self.handle_unconnected(peer_addr, packet_id, buf, now_ms) {
            return;
        }

        let mut is_disconnected = false;

        if let Some(session) = self.sessions.get_mut(&peer_addr) {
            let prev_state = session.state();
            let mut received = Vec::new();
            session.handle_packet(buf, now_ms, &mut received);

            if prev_state == SessionState::Handshake && session.state() == SessionState::Active {
                self.events.push_back(RaknetEvent::Connected {
                    peer: peer_addr,
                    client_guid: session.client_guid(),
                    mtu: session.mtu(),
                });
            }

            if session.state() == SessionState::Active {
                for payload in received {
                    self.events.push_back(RaknetEvent::Packet {
                        peer: peer_addr,
                        payload,
                    });
                }
            }

            if session.state() == SessionState::Disconnected {
                is_disconnected = true;
            }
        }

        if is_disconnected {
            self.sessions.remove(&peer_addr);
            self.events
                .push_back(RaknetEvent::Disconnected { peer: peer_addr });
        }
    }

    pub fn handle_packet(&mut self, peer_addr: SocketAddr, buf: &[u8], now_ms: u64) {
        self.accept_inbound(peer_addr, buf, now_ms);
    }

    pub fn send(
        &mut self,
        peer_addr: SocketAddr,
        payload: Bytes,
        reliability: Reliability,
        channel: u8,
    ) -> Result<(), SessionError> {
        if let Some(session) = self.sessions.get_mut(&peer_addr) {
            session.queue_frame(payload, reliability, channel)
        } else {
            Err(SessionError::Disconnected)
        }
    }

    pub fn disconnect(&mut self, peer_addr: SocketAddr) {
        if self.sessions.remove(&peer_addr).is_some() {
            self.events
                .push_back(RaknetEvent::Disconnected { peer: peer_addr });
        }
    }

    pub fn step(&mut self, now_ms: u64) {
        let mut disconnected = Vec::new();

        for (&addr, session) in &mut self.sessions {
            session.tick(now_ms);
            session.produce_outgoing_packets(now_ms, &mut self.outbound);
            if session.state() == SessionState::Disconnected {
                disconnected.push(addr);
            }
        }

        for addr in disconnected {
            self.sessions.remove(&addr);
            self.events
                .push_back(RaknetEvent::Disconnected { peer: addr });
        }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.step(now_ms);
    }

    pub fn poll_event(&mut self) -> Option<RaknetEvent> {
        self.events.pop_front()
    }

    pub fn poll_outbound_slice(&mut self, buf: &mut [u8]) -> Option<(usize, SocketAddr)> {
        let (addr, bytes) = self.outbound.pop_front()?;
        let len = bytes.len();
        if buf.len() < len {
            self.outbound.push_front((addr, bytes));
            return None;
        }
        let target = buf.get_mut(..len)?;
        target.copy_from_slice(bytes.as_ref());
        Some((len, addr))
    }

    pub fn poll_outbound(&mut self) -> Option<(SocketAddr, Bytes)> {
        self.outbound.pop_front()
    }
}
