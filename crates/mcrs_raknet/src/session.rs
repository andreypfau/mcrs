extern crate alloc;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use bytes::Bytes;
use core::net::SocketAddr;

use crate::ack::{AckPacket, AckRecord, MAX_RECORDS_PER_ACK};
use crate::connected::{
    ConnectedPing, ConnectedPong, ConnectionRequest, ConnectionRequestAccepted,
    NewIncomingConnection, SYSTEM_ADDRESSES,
};
use crate::datagram::Datagram;
use crate::fragment::{FragmentReassembler, MAX_SPLIT_COUNT};
use crate::frame::{Frame, SplitInfo};
use crate::receive_window::ReceiveWindow;
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
pub const HANDSHAKE_TIMEOUT_MS: u64 = 10_000;
pub const IDLE_TIMEOUT_MS: u64 = 30_000;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Handshake,
    Active,
    Disconnected,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SessionError {
    InvalidMtu,
    ChannelOutOfBounds,
    PayloadTooLarge,
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
    channel_send_sequences: [Uint24; MAX_ORDER_CHANNELS],
    channel_recv_sequences: [Option<Uint24>; MAX_ORDER_CHANNELS],
    channel_queues: [BTreeMap<Uint24, Bytes>; MAX_ORDER_CHANNELS],

    received_datagrams: ReceiveWindow,
    received_reliable: ReceiveWindow,

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
            channel_send_sequences: [Uint24::new(0); MAX_ORDER_CHANNELS],
            channel_recv_sequences: [None; MAX_ORDER_CHANNELS],
            channel_queues: Default::default(),
            received_datagrams: ReceiveWindow::new(),
            received_reliable: ReceiveWindow::new(),
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

    #[must_use]
    pub fn reassembly_bytes(&self) -> usize {
        self.reassembler.total_buffered_bytes()
    }

    pub fn set_reassembly_limit(&mut self, limit_bytes: usize) {
        self.reassembler.set_limit_bytes(limit_bytes);
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

            if !self.received_datagrams.accept(seq) {
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

        let Some(highest) = self.received_datagrams.highest() else {
            return;
        };

        if seq.is_newer_than(highest) {
            let mut missing = highest.wrapping_add(1);
            while missing != seq && self.pending_nacks.len() < MAX_PENDING_SEQS {
                if !self.received_datagrams.contains(missing) {
                    self.pending_nacks.push(missing);
                }
                missing = missing.wrapping_add(1);
            }
        }
    }

    fn handle_frame(&mut self, frame: Frame, now_ms: u64, received_payloads: &mut Vec<Bytes>) {
        let reliability = frame.reliability;
        if reliability.is_reliable()
            && let Some(index) = frame.reliable_index
            && !self.received_reliable.accept(index)
        {
            return;
        }

        let channel = frame.order_channel as usize;
        let ordering = reliability.is_ordered() || reliability.is_sequenced();
        if ordering && channel >= MAX_ORDER_CHANNELS {
            self.state = SessionState::Disconnected;
            return;
        }

        let payload = if let Some(split) = frame.split_info {
            match self.reassembler.insert(split, frame.payload, now_ms) {
                Ok(Some(full)) => full,
                Ok(None) => return,
                Err(_) => {
                    if reliability.is_reliable() {
                        self.state = SessionState::Disconnected;
                    }
                    return;
                }
            }
        } else {
            frame.payload
        };

        let order_index = frame.order_index.unwrap_or(Uint24::new(0));
        if reliability.is_ordered() {
            self.handle_ordered(channel, order_index, payload, received_payloads, now_ms);
        } else if reliability.is_sequenced() {
            let sequence_index = frame.sequence_index.unwrap_or(Uint24::new(0));
            self.handle_sequenced(
                channel,
                order_index,
                sequence_index,
                payload,
                received_payloads,
                now_ms,
            );
        } else {
            self.process_control_or_payload(payload, received_payloads, now_ms);
        }
    }

    fn handle_ordered(
        &mut self,
        channel: usize,
        order_index: Uint24,
        payload: Bytes,
        received_payloads: &mut Vec<Bytes>,
        now_ms: u64,
    ) {
        let Some(expected) = self.channel_recv_indices.get(channel).copied() else {
            return;
        };

        if order_index == expected {
            let mut drained = Vec::new();
            let mut next = expected.wrapping_add(1);
            if let Some(queue) = self.channel_queues.get_mut(channel) {
                while let Some(next_payload) = queue.remove(&next) {
                    next = next.wrapping_add(1);
                    drained.push(next_payload);
                }
            }
            if let Some(slot) = self.channel_recv_indices.get_mut(channel) {
                *slot = next;
            }
            if let Some(slot) = self.channel_recv_sequences.get_mut(channel) {
                *slot = None;
            }
            self.process_control_or_payload(payload, received_payloads, now_ms);
            for next_payload in drained {
                self.process_control_or_payload(next_payload, received_payloads, now_ms);
            }
        } else if order_index.is_newer_than(expected) {
            let within_window = order_index.wrapping_sub(expected) <= MAX_ORDER_WINDOW;
            if !within_window || !self.buffer_ordered_frame(channel, order_index, payload) {
                self.state = SessionState::Disconnected;
            }
        }
    }

    fn handle_sequenced(
        &mut self,
        channel: usize,
        order_index: Uint24,
        sequence_index: Uint24,
        payload: Bytes,
        received_payloads: &mut Vec<Bytes>,
        now_ms: u64,
    ) {
        let Some(expected) = self.channel_recv_indices.get(channel).copied() else {
            return;
        };
        if order_index != expected {
            return;
        }
        let Some(slot) = self.channel_recv_sequences.get_mut(channel) else {
            return;
        };
        if slot.is_some_and(|highest| !sequence_index.is_newer_than(highest)) {
            return;
        }
        *slot = Some(sequence_index);
        self.process_control_or_payload(payload, received_payloads, now_ms);
    }

    fn buffer_ordered_frame(&mut self, channel: usize, order_idx: Uint24, payload: Bytes) -> bool {
        let Some(queue) = self.channel_queues.get_mut(channel) else {
            return false;
        };
        if queue.len() >= MAX_CHANNEL_BUFFER_PER_CHANNEL && !queue.contains_key(&order_idx) {
            return false;
        }
        queue.insert(order_idx, payload);
        true
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

        let part_count = payload.len().div_ceil(max_part);
        if part_count > MAX_SPLIT_COUNT as usize {
            return Err(SessionError::PayloadTooLarge);
        }

        let (order_index, sequence_index) = self.next_ordering(channel, reliability);

        if part_count <= 1 {
            let frame = self.build_frame(reliability, channel, order_index, sequence_index, None);
            self.enqueue_frame(Frame { payload, ..frame });
            return Ok(());
        }

        let split_id = self.next_split_id;
        self.next_split_id = self.next_split_id.wrapping_add(1);

        for index in 0..part_count {
            let start = index * max_part;
            let end = core::cmp::min(start + max_part, payload.len());
            let split = SplitInfo::new(part_count as u32, split_id, index as u32);
            let frame = self.build_frame(
                reliability,
                channel,
                order_index,
                sequence_index,
                Some(split),
            );
            self.enqueue_frame(Frame {
                payload: payload.slice(start..end),
                ..frame
            });
        }

        Ok(())
    }

    fn next_ordering(
        &mut self,
        channel: u8,
        reliability: Reliability,
    ) -> (Option<Uint24>, Option<Uint24>) {
        let ch = channel as usize;
        let (Some(send_index), Some(send_sequence)) = (
            self.channel_send_indices.get_mut(ch),
            self.channel_send_sequences.get_mut(ch),
        ) else {
            return (None, None);
        };
        if reliability.is_ordered() {
            let index = *send_index;
            *send_index = index.wrapping_add(1);
            *send_sequence = Uint24::new(0);
            (Some(index), None)
        } else if reliability.is_sequenced() {
            let sequence = *send_sequence;
            *send_sequence = sequence.wrapping_add(1);
            (Some(*send_index), Some(sequence))
        } else {
            (None, None)
        }
    }

    fn build_frame(
        &mut self,
        reliability: Reliability,
        channel: u8,
        order_index: Option<Uint24>,
        sequence_index: Option<Uint24>,
        split_info: Option<SplitInfo>,
    ) -> Frame {
        let reliable_index = if reliability.is_reliable() {
            let index = self.next_reliable_index;
            self.next_reliable_index = index.wrapping_add(1);
            Some(index)
        } else {
            None
        };
        Frame {
            reliability,
            reliable_index,
            sequence_index,
            order_index,
            order_channel: channel,
            split_info,
            payload: Bytes::new(),
        }
    }

    fn enqueue_frame(&mut self, frame: Frame) {
        if frame.reliability.is_reliable() {
            self.outbound_reliable_queue.push_back(frame);
        } else {
            self.outbound_unreliable_queue.push_back(frame);
        }
    }

    pub fn tick(&mut self, now_ms: u64) {
        let idle_limit_ms = if self.state == SessionState::Handshake {
            HANDSHAKE_TIMEOUT_MS
        } else {
            IDLE_TIMEOUT_MS
        };
        if now_ms.saturating_sub(self.last_recv_ms) > idle_limit_ms {
            self.state = SessionState::Disconnected;
            self.unacked_datagrams.clear();
            return;
        }

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

    pub fn close(&mut self, outbound: &mut VecDeque<(SocketAddr, Bytes)>) {
        if self.state == SessionState::Disconnected {
            return;
        }
        let notification = Bytes::from_static(&[id::DISCONNECT_NOTIFICATION]);
        if self
            .queue_frame(notification, Reliability::Unreliable, 0)
            .is_ok()
        {
            self.produce_outgoing_packets(self.last_recv_ms, outbound);
        }
        self.state = SessionState::Disconnected;
    }
}
