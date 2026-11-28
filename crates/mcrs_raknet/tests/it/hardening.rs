#![expect(clippy::unwrap_used, reason = "test helpers may panic")]

use bytes::Bytes;
use core::net::SocketAddr;
use mcrs_raknet::{
    AckPacket, AddressCodec, Datagram, FragmentError, FragmentReassembler, Frame,
    HANDSHAKE_TIMEOUT_MS, Motd, OpenConnectionReply2, OpenConnectionRequest2, PART_OVERHEAD_BYTES,
    RaknetServer, RaknetSession, Reliability, SessionError, SessionState, SplitInfo, Uint24,
    packet_id,
};

const GAME_PAYLOAD: &[u8] = b"\xfegame";

fn peer() -> SocketAddr {
    "127.0.0.1:19132".parse().unwrap()
}

fn datagram_bytes(seq: u32, frame: Frame) -> Vec<u8> {
    let mut datagram = Datagram::new(Uint24::new(seq));
    datagram.push_frame(frame);
    let mut buf = vec![0u8; datagram.wire_size()];
    let len = datagram.write_to(&mut buf).unwrap();
    buf.truncate(len);
    buf
}

fn deliver(session: &mut RaknetSession, seq: u32, frame: Frame) -> usize {
    let mut received = Vec::new();
    session.handle_packet(&datagram_bytes(seq, frame), 1000, &mut received);
    received.len()
}

#[test]
fn ipv4_octets_are_inverted_on_the_wire() {
    let addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let mut buf = [0u8; 7];
    assert_eq!(AddressCodec::write_to(addr, &mut buf), Some(7));
    assert_eq!(buf, [4, 0x80, 0xFF, 0xFF, 0xFE, 0x4A, 0xBC]);
    let (decoded, rest) = AddressCodec::read_from(&buf).unwrap();
    assert_eq!(decoded, addr);
    assert!(rest.is_empty());
}

#[test]
fn ipv6_flow_info_and_scope_id_survive_a_round_trip() {
    let addr = SocketAddr::V6(core::net::SocketAddrV6::new(
        "fe80::1".parse().unwrap(),
        19132,
        0x0102_0304,
        7,
    ));
    let mut buf = [0u8; 29];
    assert_eq!(AddressCodec::write_to(addr, &mut buf), Some(29));
    let (decoded, _) = AddressCodec::read_from(&buf).unwrap();
    assert_eq!(decoded, addr);
}

#[test]
fn reply2_without_room_for_the_security_byte_is_not_written() {
    let reply = OpenConnectionReply2 {
        server_guid: 1,
        client_address: peer(),
        mtu_size: 1400,
        security: false,
    };
    assert_eq!(reply.write_to(&mut [0u8; 34]), None);
    let mut exact = [0u8; 35];
    let len = reply.write_to(&mut exact).unwrap();
    assert_eq!(OpenConnectionReply2::read_from(&exact[..len]), Some(reply));
}

#[test]
fn ack_keeps_every_sequence_beyond_the_record_limit() {
    let sequences: Vec<Uint24> = (0..600).map(|i| Uint24::new(i * 2)).collect();
    let ack = AckPacket::from_sequences(&sequences, false);
    assert_eq!(ack.records().len(), sequences.len());
}

#[test]
fn frame_without_required_indices_is_not_written() {
    let frame = Frame::new(
        Reliability::ReliableOrdered,
        Bytes::from_static(GAME_PAYLOAD),
    );
    assert!(frame.write_to(&mut [0u8; 64]).is_none());
}

#[test]
fn empty_fragments_are_charged_against_the_budget() {
    let mut reassembler = FragmentReassembler::default();
    reassembler.set_limit_bytes(PART_OVERHEAD_BYTES * 10);
    let mut accepted = 0;
    let mut outcome = Ok(None);
    for index in 0..100 {
        outcome = reassembler.insert(SplitInfo::new(8192, 1, index), Bytes::new(), 0);
        if outcome.is_err() {
            break;
        }
        accepted += 1;
    }
    assert_eq!(outcome, Err(FragmentError::MemoryLimitExceeded));
    assert_eq!(accepted, 10);
}

#[test]
fn payload_beyond_the_split_limit_is_refused() {
    let mut session = RaknetSession::new(peer(), 576, 1, 0).unwrap();
    let payload = Bytes::from(vec![0u8; 512 * 8193]);
    assert_eq!(
        session.queue_frame(payload, Reliability::ReliableOrdered, 0),
        Err(SessionError::PayloadTooLarge)
    );
}

#[test]
fn resent_reliable_frame_in_a_new_datagram_is_delivered_once() {
    let mut session = RaknetSession::new(peer(), 1400, 1, 0).unwrap();
    let frame = || {
        Frame::new(Reliability::Reliable, Bytes::from_static(GAME_PAYLOAD))
            .with_reliable_index(Uint24::new(3))
    };
    assert_eq!(deliver(&mut session, 0, frame()), 1);
    assert_eq!(deliver(&mut session, 1, frame()), 0);
}

#[test]
fn sequenced_frames_do_not_consume_the_ordering_index() {
    let mut session = RaknetSession::new(peer(), 1400, 1, 0).unwrap();
    let sequenced = |order: u32, sequence: u32| {
        Frame::new(
            Reliability::UnreliableSequenced,
            Bytes::from_static(GAME_PAYLOAD),
        )
        .with_sequence_index(Uint24::new(sequence))
        .with_order(Uint24::new(order), 0)
    };
    let ordered = Frame::new(
        Reliability::ReliableOrdered,
        Bytes::from_static(GAME_PAYLOAD),
    )
    .with_reliable_index(Uint24::new(0))
    .with_order(Uint24::new(0), 0);

    assert_eq!(deliver(&mut session, 0, sequenced(0, 0)), 1);
    assert_eq!(deliver(&mut session, 1, sequenced(0, 0)), 0);
    assert_eq!(deliver(&mut session, 2, ordered), 1);
    assert_eq!(deliver(&mut session, 3, sequenced(1, 0)), 1);
}

#[test]
fn ordered_frame_beyond_the_window_ends_the_session() {
    let mut session = RaknetSession::new(peer(), 1400, 1, 0).unwrap();
    let frame = Frame::new(
        Reliability::ReliableOrdered,
        Bytes::from_static(GAME_PAYLOAD),
    )
    .with_reliable_index(Uint24::new(0))
    .with_order(Uint24::new(5000), 0);
    deliver(&mut session, 0, frame);
    assert_eq!(session.state(), SessionState::Disconnected);
}

#[test]
fn silent_handshake_session_times_out() {
    let mut session = RaknetSession::new(peer(), 1400, 1, 1000).unwrap();
    session.tick(1000 + HANDSHAKE_TIMEOUT_MS);
    assert_eq!(session.state(), SessionState::Handshake);
    session.tick(1001 + HANDSHAKE_TIMEOUT_MS);
    assert_eq!(session.state(), SessionState::Disconnected);
}

#[test]
fn server_disconnect_tells_the_peer() {
    let mut server = RaknetServer::new(1, Motd::default());
    let request = OpenConnectionRequest2 {
        server_address: peer(),
        mtu_size: 1400,
        client_guid: 9,
    };
    let mut buf = [0u8; 128];
    let len = request.write_to(&mut buf).unwrap();
    server.accept_inbound(peer(), &buf[..len], 0);
    let reply = server.poll_outbound().unwrap().1;
    assert_eq!(reply[0], packet_id::OPEN_CONNECTION_REPLY_2);

    server.disconnect(peer());
    let (target, datagram) = server.poll_outbound().unwrap();
    assert_eq!(target, peer());
    let decoded = Datagram::read_from(&datagram).unwrap();
    assert_eq!(
        decoded.frames()[0].payload().as_ref(),
        &[packet_id::DISCONNECT_NOTIFICATION]
    );
}
