extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;
use bytes::Bytes;
use core::net::SocketAddr;

use crate::ack::{AckPacket, AckRecord, MAX_RECORDS_PER_ACK};
use crate::connected::{
    ConnectedPing, ConnectedPong, ConnectionRequest, ConnectionRequestAccepted,
    NewIncomingConnection, SYSTEM_ADDRESSES,
};
use crate::datagram::Datagram;
use crate::fragment::FragmentReassembler;
use crate::frame::Frame;
use crate::reliability::Reliability;
use crate::unconnected::id;
use crate::wire::Uint24;

pub const MIN_MTU: u16 = 576;
pub const MAX_MTU: u16 = 1492;
pub const MAX_ORDER_CHANNELS: usize = 32;
pub const MAX_ORDER_WINDOW: u32 = 1024;
pub const MAX_CHANNEL_BUFFER_PER_CHANNEL: usize = 256;
pub const MAX_RETRANSMIT_ATTEMPTS: u8 = 10;
pub const MAX_IN_FLIGHT_DATAGRAMS: usize = 64;
pub const MAX_PENDING_SEQS: usize = 512;
pub const MAX_DEDUP_HISTORY: usize = 2048;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Handshake,
    Active,
    Disconnected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionError {
    InvalidMtu,
    ChannelOutOfBounds,
    Disconnected,
}

#[derive(Clone, Debug)]
pub(crate) struct OutgoingDatagram {
    pub(crate) datagram: Datagram,
    pub(crate) sent_at_ms: u64,
    pub(crate) attempts: u8,
}

pub struct RaknetSession {
    peer_addr: SocketAddr,
    mtu: u16,
    state: SessionState,
    client_guid: u64,

    next_send_seq: Uint24,
    next_reliable_index: Uint24,
    next_split_id: u16,

    channel_send_indices: [Uint24; MAX_ORDER_CHANNELS],
    channel_recv_indices: [Uint24; MAX_ORDER_CHANNELS],
    channel_queues: [BTreeMap<Uint24, Bytes>; MAX_ORDER_CHANNELS],

    received_seq_history: BTreeSet<Uint24>,
    highest_recv_seq: Option<Uint24>,

    pending_incoming_seqs: Vec<Uint24>,
    pending_nacks: Vec<Uint24>,
    unacked_datagrams: BTreeMap<Uint24, OutgoingDatagram>,
    outbound_reliable_queue: VecDeque<Frame>,
    outbound_unreliable_queue: VecDeque<Frame>,

    reassembler: FragmentReassembler,

    last_recv_ms: u64,
    rtt_ms: u64,
}

impl RaknetSession {
    pub fn new(
        peer_addr: SocketAddr,
        mtu: u16,
        _server_guid: u64,
        now_ms: u64,
    ) -> Result<Self, SessionError> {
        if !(MIN_MTU..=MAX_MTU).contains(&mtu) {
            return Err(SessionError::InvalidMtu);
        }
        Ok(Self {
            peer_addr,
            mtu,
            state: SessionState::Handshake,
            client_guid: 0,
            next_send_seq: Uint24::new(0),
            next_reliable_index: Uint24::new(0),
            next_split_id: 0,
            channel_send_indices: [Uint24::new(0); MAX_ORDER_CHANNELS],
            channel_recv_indices: [Uint24::new(0); MAX_ORDER_CHANNELS],
            channel_queues: Default::default(),
            received_seq_history: BTreeSet::new(),
            highest_recv_seq: None,
            pending_incoming_seqs: Vec::new(),
            pending_nacks: Vec::new(),
            unacked_datagrams: BTreeMap::new(),
            outbound_reliable_queue: VecDeque::new(),
            outbound_unreliable_queue: VecDeque::new(),
            reassembler: FragmentReassembler::default(),
            last_recv_ms: now_ms,
            rtt_ms: 100,
        })
    }

    #[must_use]
    pub const fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }

    #[must_use]
    pub const fn mtu(&self) -> u16 {
        self.mtu
    }

    #[must_use]
    pub const fn state(&self) -> SessionState {
        self.state
    }

    #[must_use]
    pub const fn client_guid(&self) -> u64 {
        self.client_guid
    }

    pub fn set_client_guid(&mut self, guid: u64) {
        self.client_guid = guid;
    }

    #[inline]
    #[must_use]
    pub fn max_payload_per_frame(&self) -> usize {
        (self.mtu as usize).saturating_sub(64)
    }

    pub fn handle_packet(
        &mut self,
        buf: &[u8],
        now_ms: u64,
        received_payloads: &mut Vec<Bytes>,
    ) -> Option<()> {
        let header = *buf.first()?;
        self.last_recv_ms = now_ms;

        if header == id::ACK || header == id::NACK {
            let ack = AckPacket::read_from(buf)?;
            self.handle_ack_packet(ack, now_ms);
            return Some(());
        }

        if header == id::DISCONNECT_NOTIFICATION {
            self.state = SessionState::Disconnected;
            return Some(());
        }

        if (id::FRAME_SET_MIN..=id::FRAME_SET_MAX).contains(&header) {
            let datagram = Datagram::read_from(buf)?;
            let seq = datagram.seq_number;

            self.record_incoming_sequence(seq);

            if !self.track_received_dedup(seq) {
                return Some(());
            }

            for frame in datagram.frames {
                self.handle_frame(frame, now_ms, received_payloads);
            }
            return Some(());
        }

        None
    }

    fn record_incoming_sequence(&mut self, seq: Uint24) {
        if self.pending_incoming_seqs.len() < MAX_PENDING_SEQS {
            self.pending_incoming_seqs.push(seq);
        }

        let Some(highest) = self.highest_recv_seq else {
            self.highest_recv_seq = Some(seq);
            return;
        };

        if seq.is_newer_than(highest) {
            let mut missing = highest.wrapping_add(1);
            while missing != seq && self.pending_nacks.len() < MAX_PENDING_SEQS {
                if !self.received_seq_history.contains(&missing) {
                    self.pending_nacks.push(missing);
                }
                missing = missing.wrapping_add(1);
            }
            self.highest_recv_seq = Some(seq);
        }
    }

    fn track_received_dedup(&mut self, seq: Uint24) -> bool {
        if self.received_seq_history.contains(&seq) {
            return false;
        }
        self.received_seq_history.insert(seq);
        if self.received_seq_history.len() > MAX_DEDUP_HISTORY {
            let oldest = self.received_seq_history.iter().next().copied();
            if let Some(oldest) = oldest {
                self.received_seq_history.remove(&oldest);
            }
        }
        true
    }

    fn handle_frame(&mut self, frame: Frame, now_ms: u64, received_payloads: &mut Vec<Bytes>) {
        let payload = if let Some(split) = frame.split_info {
            match self.reassembler.insert(split, frame.payload, now_ms) {
                Ok(Some(full)) => full,
                _ => return,
            }
        } else {
            frame.payload
        };

        if frame.reliability.is_ordered() || frame.reliability.is_sequenced() {
            let channel = frame.order_channel as usize;
            if channel >= MAX_ORDER_CHANNELS {
                return;
            }

            let order_idx = frame.order_index.unwrap_or(Uint24::new(0));
            let expected_idx = self
                .channel_recv_indices
                .get(channel)
                .copied()
                .unwrap_or(Uint24::new(0));

            if order_idx == expected_idx {
                if let Some(slot) = self.channel_recv_indices.get_mut(channel) {
                    *slot = expected_idx.wrapping_add(1);
                }
                self.process_control_or_payload(payload, received_payloads, now_ms);

                let mut drained = Vec::new();
                if let Some(queue) = self.channel_queues.get_mut(channel) {
                    let mut current_expected = self
                        .channel_recv_indices
                        .get(channel)
                        .copied()
                        .unwrap_or(Uint24::new(0));
                    while let Some(next_payload) = queue.remove(&current_expected) {
                        current_expected = current_expected.wrapping_add(1);
                        drained.push(next_payload);
                    }
                    if let Some(slot) = self.channel_recv_indices.get_mut(channel) {
                        *slot = current_expected;
                    }
                }
                for next_payload in drained {
                    self.process_control_or_payload(next_payload, received_payloads, now_ms);
                }
            } else if order_idx.is_newer_than(expected_idx) {
                let diff = order_idx.wrapping_sub(expected_idx);
                if diff <= MAX_ORDER_WINDOW {
                    self.buffer_ordered_frame(channel, order_idx, payload);
                }
            }
        } else {
            self.process_control_or_payload(payload, received_payloads, now_ms);
        }
    }

    fn buffer_ordered_frame(&mut self, channel: usize, order_idx: Uint24, payload: Bytes) {
        let Some(queue) = self.channel_queues.get_mut(channel) else {
            return;
        };
        if queue.len() < MAX_CHANNEL_BUFFER_PER_CHANNEL {
            queue.insert(order_idx, payload);
        }
    }

    fn process_control_or_payload(
        &mut self,
        payload: Bytes,
        received_payloads: &mut Vec<Bytes>,
        now_ms: u64,
    ) {
        let Some(&id) = payload.first() else {
            return;
        };

        match id {
            id::CONNECTION_REQUEST => {
                if let Some(req) = ConnectionRequest::read_from(&payload) {
                    self.client_guid = req.client_guid;
                    let accept = ConnectionRequestAccepted {
                        client_address: self.peer_addr,
                        system_index: 0,
                        system_addresses: SYSTEM_ADDRESSES,
                        request_time: req.request_time,
                        server_time: now_ms as i64,
                    };
                    let mut accept_buf = [0u8; 256];
                    if let Some(slice) = accept
                        .write_to(&mut accept_buf)
                        .and_then(|len| accept_buf.get(..len))
                    {
                        let _ = self.queue_frame(
                            Bytes::copy_from_slice(slice),
                            Reliability::ReliableOrdered,
                            0,
                        );
                    }
                }
            }
            id::NEW_INCOMING_CONNECTION => {
                let is_nic = NewIncomingConnection::read_from(&payload).is_some();
                if is_nic && self.state == SessionState::Handshake {
                    self.state = SessionState::Active;
                }
            }
            id::CONNECTED_PING => {
                if let Some(ping) = ConnectedPing::read_from(&payload) {
                    let pong = ConnectedPong {
                        client_time: ping.client_time,
                        server_time: now_ms as i64,
                    };
                    let mut buf = [0u8; 17];
                    if let Some(slice) = pong.write_to(&mut buf).and_then(|len| buf.get(..len)) {
                        let _ = self.queue_frame(
                            Bytes::copy_from_slice(slice),
                            Reliability::Reliable,
                            0,
                        );
                    }
                }
            }
            id::CONNECTED_PONG => {}
            id::DISCONNECT_NOTIFICATION => {
                self.state = SessionState::Disconnected;
            }
            _ => {
                received_payloads.push(payload);
            }
        }
    }

    fn on_ack_received(&mut self, seq: Uint24, now_ms: u64) {
        let Some(out) = self.unacked_datagrams.remove(&seq) else {
            return;
        };
        if out.sent_at_ms > 0 {
            let sample_rtt = now_ms.saturating_sub(out.sent_at_ms);
            self.rtt_ms = (self.rtt_ms * 7 + sample_rtt) / 8;
        }
    }

    fn handle_ack_packet(&mut self, ack: AckPacket, now_ms: u64) {
        if !ack.is_nack() {
            for rec in ack.records() {
                match *rec {
                    AckRecord::Single(seq) => {
                        self.on_ack_received(seq, now_ms);
                    }
                    AckRecord::Range(start, end) => {
                        let to_remove: Vec<Uint24> = self
                            .unacked_datagrams
                            .keys()
                            .copied()
                            .filter(|&k| Uint24::is_in_range_wrapped(k, start, end))
                            .collect();
                        for k in to_remove {
                            self.on_ack_received(k, now_ms);
                        }
                    }
                }
            }
        } else {
            for rec in ack.records() {
                let to_retransmit: Vec<Uint24> = self
                    .unacked_datagrams
                    .keys()
                    .copied()
                    .filter(|&k| rec.contains(k))
                    .collect();
                for k in to_retransmit {
                    if let Some(item) = self.unacked_datagrams.get_mut(&k) {
                        item.sent_at_ms = 0;
                    }
                }
            }
        }
    }

    pub fn queue_frame(
        &mut self,
        payload: Bytes,
        reliability: Reliability,
        channel: u8,
    ) -> Result<(), SessionError> {
        if channel as usize >= MAX_ORDER_CHANNELS {
            return Err(SessionError::ChannelOutOfBounds);
        }

        let max_part = self.max_payload_per_frame();
        if max_part == 0 {
            return Err(SessionError::InvalidMtu);
        }

        let ch = channel as usize;

        if payload.len() <= max_part {
            let reliable_index = if reliability.is_reliable() {
                let idx = self.next_reliable_index;
                self.next_reliable_index = self.next_reliable_index.wrapping_add(1);
                Some(idx)
            } else {
                None
            };

            let order_index = if reliability.is_ordered() || reliability.is_sequenced() {
                let idx = self
                    .channel_send_indices
                    .get(ch)
                    .copied()
                    .unwrap_or(Uint24::new(0));
                if let Some(slot) = self.channel_send_indices.get_mut(ch) {
                    *slot = idx.wrapping_add(1);
                }
                Some(idx)
            } else {
                None
            };

            let frame = Frame {
                reliability,
                reliable_index,
                sequence_index: None,
                order_index,
                order_channel: channel,
                split_info: None,
                payload,
            };

            if reliability.is_reliable() {
                self.outbound_reliable_queue.push_back(frame);
            } else {
                self.outbound_unreliable_queue.push_back(frame);
            }
        } else {
            let split_id = self.next_split_id;
            self.next_split_id = self.next_split_id.wrapping_add(1);

            let splits = FragmentReassembler::split_payload(&payload, split_id, max_part);
            let mut offset = 0;

            let order_index = if reliability.is_ordered() || reliability.is_sequenced() {
                let idx = self
                    .channel_send_indices
                    .get(ch)
                    .copied()
                    .unwrap_or(Uint24::new(0));
                if let Some(slot) = self.channel_send_indices.get_mut(ch) {
                    *slot = idx.wrapping_add(1);
                }
                Some(idx)
            } else {
                None
            };

            for split in splits {
                let chunk_len = core::cmp::min(max_part, payload.len().saturating_sub(offset));
                let chunk = payload.slice(offset..offset + chunk_len);
                offset += chunk_len;

                let reliable_index = if reliability.is_reliable() {
                    let idx = self.next_reliable_index;
                    self.next_reliable_index = self.next_reliable_index.wrapping_add(1);
                    Some(idx)
                } else {
                    None
                };

                let frame = Frame {
                    reliability,
                    reliable_index,
                    sequence_index: None,
                    order_index,
                    order_channel: channel,
                    split_info: Some(split),
                    payload: chunk,
                };

                if reliability.is_reliable() {
                    self.outbound_reliable_queue.push_back(frame);
                } else {
                    self.outbound_unreliable_queue.push_back(frame);
                }
            }
        }

        Ok(())
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.reassembler.prune(now_ms);

        let rto_ms = core::cmp::max(self.rtt_ms * 2, 250);
        let mut timed_out = false;

        for item in self.unacked_datagrams.values_mut() {
            let elapsed = now_ms.saturating_sub(item.sent_at_ms);
            if elapsed > rto_ms {
                if item.attempts >= MAX_RETRANSMIT_ATTEMPTS {
                    timed_out = true;
                    break;
                }
                item.sent_at_ms = 0;
            }
        }

        if timed_out {
            self.state = SessionState::Disconnected;
            self.unacked_datagrams.clear();
        }
    }

    fn write_and_queue(
        peer_addr: SocketAddr,
        writer: impl FnOnce(&mut [u8]) -> Option<usize>,
        needed: usize,
        scratch: &mut [u8; 2048],
        outbound: &mut VecDeque<(SocketAddr, Bytes)>,
    ) {
        let Some(buf) = scratch.get_mut(..needed) else {
            return;
        };
        let Some(len) = writer(buf) else {
            return;
        };
        let Some(slice) = scratch.get(..len) else {
            return;
        };
        outbound.push_back((peer_addr, Bytes::copy_from_slice(slice)));
    }

    fn flush_acks(
        &mut self,
        scratch: &mut [u8; 2048],
        outbound: &mut VecDeque<(SocketAddr, Bytes)>,
    ) {
        while !self.pending_incoming_seqs.is_empty() {
            let chunk_size = self.pending_incoming_seqs.len().min(MAX_RECORDS_PER_ACK);
            let chunk: Vec<Uint24> = self.pending_incoming_seqs.drain(..chunk_size).collect();
            let ack_pkt = AckPacket::from_sequences(&chunk, false);
            Self::write_and_queue(
                self.peer_addr,
                |buf| ack_pkt.write_to(buf),
                ack_pkt.wire_size(),
                scratch,
                outbound,
            );
        }

        while !self.pending_nacks.is_empty() {
            let chunk_size = self.pending_nacks.len().min(MAX_RECORDS_PER_ACK);
            let chunk: Vec<Uint24> = self.pending_nacks.drain(..chunk_size).collect();
            let nack_pkt = AckPacket::from_sequences(&chunk, true);
            Self::write_and_queue(
                self.peer_addr,
                |buf| nack_pkt.write_to(buf),
                nack_pkt.wire_size(),
                scratch,
                outbound,
            );
        }
    }

    fn flush_retransmissions(
        &mut self,
        now_ms: u64,
        scratch: &mut [u8; 2048],
        outbound: &mut VecDeque<(SocketAddr, Bytes)>,
    ) {
        let rto_ms = core::cmp::max(self.rtt_ms * 2, 250);
        let mut retransmits: Vec<Uint24> = Vec::new();
        for (&seq, item) in &self.unacked_datagrams {
            let is_stale = item.sent_at_ms == 0 || now_ms.saturating_sub(item.sent_at_ms) > rto_ms;
            if is_stale && item.attempts < MAX_RETRANSMIT_ATTEMPTS {
                retransmits.push(seq);
            }
        }

        for seq in retransmits {
            if let Some(item) = self.unacked_datagrams.get_mut(&seq) {
                let needed = item.datagram.wire_size();
                Self::write_and_queue(
                    self.peer_addr,
                    |buf| item.datagram.write_to(buf),
                    needed,
                    scratch,
                    outbound,
                );
                item.sent_at_ms = now_ms;
                item.attempts += 1;
            }
        }
    }

    fn flush_reliable_queue(
        &mut self,
        now_ms: u64,
        scratch: &mut [u8; 2048],
        outbound: &mut VecDeque<(SocketAddr, Bytes)>,
    ) {
        let max_payload = self.max_payload_per_frame();
        while self.unacked_datagrams.len() < MAX_IN_FLIGHT_DATAGRAMS
            && !self.outbound_reliable_queue.is_empty()
        {
            let seq = self.next_send_seq;
            self.next_send_seq = self.next_send_seq.wrapping_add(1);

            let mut datagram = Datagram::new(seq);
            let mut curr_size = 4;

            while let Some(front) = self.outbound_reliable_queue.front() {
                let frame_size = front.wire_size();
                if curr_size + frame_size > max_payload && !datagram.frames.is_empty() {
                    break;
                }
                if let Some(frame) = self.outbound_reliable_queue.pop_front() {
                    curr_size += frame.wire_size();
                    datagram.frames.push(frame);
                }
            }

            if datagram.frames.is_empty() {
                break;
            }

            let needed = datagram.wire_size();
            Self::write_and_queue(
                self.peer_addr,
                |buf| datagram.write_to(buf),
                needed,
                scratch,
                outbound,
            );

            self.unacked_datagrams.insert(
                seq,
                OutgoingDatagram {
                    datagram,
                    sent_at_ms: now_ms,
                    attempts: 1,
                },
            );
        }
    }

    fn flush_unreliable_queue(
        &mut self,
        scratch: &mut [u8; 2048],
        outbound: &mut VecDeque<(SocketAddr, Bytes)>,
    ) {
        let max_payload = self.max_payload_per_frame();
        while !self.outbound_unreliable_queue.is_empty() {
            let seq = self.next_send_seq;
            self.next_send_seq = self.next_send_seq.wrapping_add(1);

            let mut datagram = Datagram::new(seq);
            let mut curr_size = 4;

            while let Some(front) = self.outbound_unreliable_queue.front() {
                let frame_size = front.wire_size();
                if curr_size + frame_size > max_payload && !datagram.frames.is_empty() {
                    break;
                }
                if let Some(frame) = self.outbound_unreliable_queue.pop_front() {
                    curr_size += frame.wire_size();
                    datagram.frames.push(frame);
                }
            }

            if datagram.frames.is_empty() {
                break;
            }

            let needed = datagram.wire_size();
            Self::write_and_queue(
                self.peer_addr,
                |buf| datagram.write_to(buf),
                needed,
                scratch,
                outbound,
            );
        }
    }

    pub fn produce_outgoing_packets(
        &mut self,
        now_ms: u64,
        outbound: &mut VecDeque<(SocketAddr, Bytes)>,
    ) {
        if self.state == SessionState::Disconnected {
            return;
        }

        let mut scratch = [0u8; 2048];
        self.flush_acks(&mut scratch, outbound);
        self.flush_retransmissions(now_ms, &mut scratch, outbound);
        self.flush_reliable_queue(now_ms, &mut scratch, outbound);
        self.flush_unreliable_queue(&mut scratch, outbound);
    }
}
