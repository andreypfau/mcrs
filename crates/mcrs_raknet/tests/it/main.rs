extern crate alloc;

use bytes::Bytes;
use core::net::SocketAddr;
use mcrs_raknet::{
    AckPacket, AckRecord, ConnectedPing, ConnectedPong, Datagram, FragmentError,
    FragmentReassembler, Frame, MAX_ACTIVE_SPLITS, MAX_ORDER_CHANNELS, MAX_SESSIONS,
    MAX_SPLIT_COUNT, MIN_MTU, Motd, NewIncomingConnection, OpenConnectionRequest2, RaknetEvent,
    RaknetServer, RaknetSession, Reliability, SessionError, SplitInfo, Uint24,
};
use mcrs_test_alloc::CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn uint24_arithmetic_and_codecs() {
    let mut buf = [0u8; 3];
    let num = Uint24::new(0x12_34_56);
    assert_eq!(num.as_u32(), 0x12_34_56);

    num.write_to(&mut buf).unwrap();
    assert_eq!(buf, [0x56, 0x34, 0x12]);

    let (decoded, rest) = Uint24::read_from(&buf).unwrap();
    assert_eq!(decoded, num);
    assert!(rest.is_empty());

    let max = Uint24::MAX;
    assert_eq!(max.wrapping_add(1), Uint24::new(0));

    let a = Uint24::new(10);
    let b = Uint24::new(5);
    assert!(a.is_newer_than(b));
    assert!(!b.is_newer_than(a));

    let wrapped_high = Uint24::new(0x00FF_FFF0);
    let wrapped_low = Uint24::new(0x0000_0010);
    assert!(wrapped_low.is_newer_than(wrapped_high));
}

#[test]
fn uint24_wraparound_ordering_and_ack_ranges() {
    let start = Uint24::new(0x00FF_FFF0);
    let end = Uint24::new(0x0000_0010);

    assert!(Uint24::is_in_range_wrapped(
        Uint24::new(0x00FF_FFF5),
        start,
        end
    ));
    assert!(Uint24::is_in_range_wrapped(
        Uint24::new(0x0000_0005),
        start,
        end
    ));
    assert!(!Uint24::is_in_range_wrapped(
        Uint24::new(0x0000_0100),
        start,
        end
    ));

    let rec = AckRecord::Range(start, end);
    assert!(rec.contains(Uint24::new(0x00FF_FFFF)));
    assert!(rec.contains(Uint24::new(0x0000_0000)));
    assert!(!rec.contains(Uint24::new(0x0010_0000)));

    let seqs = [
        Uint24::new(1),
        Uint24::new(2),
        Uint24::new(3),
        Uint24::new(5),
    ];
    let ack = AckPacket::from_sequences(&seqs, false);
    assert_eq!(ack.records().len(), 2);
    assert_eq!(
        ack.records()[0],
        AckRecord::Range(Uint24::new(1), Uint24::new(3))
    );
    assert_eq!(ack.records()[1], AckRecord::Single(Uint24::new(5)));
}

#[test]
fn split_count_above_cap_rejected() {
    let mut reassembler = FragmentReassembler::default();
    let split = SplitInfo::new(MAX_SPLIT_COUNT + 1, 1, 0);
    let res = reassembler.insert(split, Bytes::from_static(b"data"), 1000);
    assert_eq!(res, Err(FragmentError::InvalidCount));

    let split_zero = SplitInfo::new(0, 1, 0);
    assert_eq!(
        reassembler.insert(split_zero, Bytes::from_static(b"data"), 1000),
        Err(FragmentError::InvalidCount)
    );

    let split_oob = SplitInfo::new(10, 1, 10);
    assert_eq!(
        reassembler.insert(split_oob, Bytes::from_static(b"data"), 1000),
        Err(FragmentError::InvalidIndex)
    );
}

#[test]
fn split_id_flood_rejected() {
    let mut reassembler = FragmentReassembler::default();
    for i in 0..MAX_ACTIVE_SPLITS {
        let split = SplitInfo::new(2, i as u16, 0);
        let res = reassembler.insert(split, Bytes::from_static(b"part0"), 1000);
        assert!(res.is_ok());
    }

    let overflow_split = SplitInfo::new(2, (MAX_ACTIVE_SPLITS + 1) as u16, 0);
    let res = reassembler.insert(overflow_split, Bytes::from_static(b"overflow"), 1000);
    assert_eq!(res, Err(FragmentError::TooManyActiveSplits));
}

#[test]
fn sequenced_round_trip() {
    let frame = Frame::new(
        Reliability::ReliableSequenced,
        Bytes::from_static(b"sequenced payload"),
    )
    .with_reliable_index(Uint24::new(100))
    .with_sequence_index(Uint24::new(200))
    .with_order(Uint24::new(300), 5);

    let mut buf = [0u8; 128];
    let total = buf.len();
    let rest = frame.write_to(&mut buf).unwrap();
    let written = total - rest.len();

    let (decoded, remaining) = Frame::read_from(&buf[..written]).unwrap();
    assert!(remaining.is_empty());
    assert_eq!(decoded.reliability(), Reliability::ReliableSequenced);
    assert_eq!(decoded.reliable_index(), Some(Uint24::new(100)));
    assert_eq!(decoded.sequence_index(), Some(Uint24::new(200)));
    assert_eq!(decoded.order_index(), Some(Uint24::new(300)));
    assert_eq!(decoded.order_channel(), 5);
    assert_eq!(decoded.payload(), &Bytes::from_static(b"sequenced payload"));
}

#[test]
fn channel_ge_32_rejected() {
    let peer: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let mut session = RaknetSession::new(peer, 1400, 1, 1000).unwrap();

    let res = session.queue_frame(
        Bytes::from_static(b"bad channel"),
        Reliability::ReliableOrdered,
        32,
    );
    assert_eq!(res, Err(SessionError::ChannelOutOfBounds));

    let res_max = session.queue_frame(
        Bytes::from_static(b"bad channel"),
        Reliability::ReliableOrdered,
        255,
    );
    assert_eq!(res_max, Err(SessionError::ChannelOutOfBounds));

    let res_valid = session.queue_frame(
        Bytes::from_static(b"good channel"),
        Reliability::ReliableOrdered,
        (MAX_ORDER_CHANNELS - 1) as u8,
    );
    assert!(res_valid.is_ok());
}

#[test]
fn mtu_bounds_checked() {
    let peer: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    assert_eq!(
        RaknetSession::new(peer, 64, 1, 1000).map(|_| ()),
        Err(SessionError::InvalidMtu)
    );
    assert_eq!(
        RaknetSession::new(peer, MIN_MTU - 1, 1, 1000).map(|_| ()),
        Err(SessionError::InvalidMtu)
    );
    assert_eq!(
        RaknetSession::new(peer, 1500, 1, 1000).map(|_| ()),
        Err(SessionError::InvalidMtu)
    );
    assert!(RaknetSession::new(peer, 1400, 1, 1000).is_ok());
}

#[test]
fn duplicate_datagram_dedup() {
    let peer: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let mut session = RaknetSession::new(peer, 1400, 1, 1000).unwrap();

    let mut datagram = Datagram::new(Uint24::new(42));
    datagram.push_frame(Frame::new(
        Reliability::Unreliable,
        Bytes::from_static(b"game packet"),
    ));

    let mut buf = [0u8; 128];
    let len = datagram.write_to(&mut buf).unwrap();

    let mut received = Vec::new();
    session.handle_packet(&buf[..len], 1000, &mut received);
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], Bytes::from_static(b"game packet"));

    let mut received2 = Vec::new();
    session.handle_packet(&buf[..len], 1001, &mut received2);
    assert_eq!(received2.len(), 0);
}

#[test]
fn nack_generation_and_retransmission() {
    let peer: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let mut session = RaknetSession::new(peer, 1400, 1, 1000).unwrap();

    let mut dg0 = Datagram::new(Uint24::new(0));
    dg0.push_frame(Frame::new(
        Reliability::Unreliable,
        Bytes::from_static(b"p0"),
    ));
    let mut buf0 = [0u8; 128];
    let len0 = dg0.write_to(&mut buf0).unwrap();

    let mut dg2 = Datagram::new(Uint24::new(2));
    dg2.push_frame(Frame::new(
        Reliability::Unreliable,
        Bytes::from_static(b"p2"),
    ));
    let mut buf2 = [0u8; 128];
    let len2 = dg2.write_to(&mut buf2).unwrap();

    let mut rcv = Vec::new();
    session.handle_packet(&buf0[..len0], 1000, &mut rcv);
    session.handle_packet(&buf2[..len2], 1001, &mut rcv);

    let mut outbound = alloc::collections::VecDeque::new();
    session.produce_outgoing_packets(1002, &mut outbound);

    let nack_found = outbound.iter().any(|(_, bytes)| {
        if let Some(ack) = AckPacket::read_from(bytes) {
            ack.is_nack() && ack.records().iter().any(|r| r.contains(Uint24::new(1)))
        } else {
            false
        }
    });
    assert!(nack_found);
}

#[test]
fn ocr2_session_hijack_rejected() {
    let peer: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let mut server = RaknetServer::new(0x1234, Motd::default());

    let ocr2 = OpenConnectionRequest2 {
        server_address: peer,
        mtu_size: 1400,
        client_guid: 0x9999,
    };
    let mut buf = [0u8; 128];
    let len = ocr2.write_to(&mut buf).unwrap();

    server.accept_inbound(peer, &buf[..len], 1000);
    assert_eq!(server.session_count(), 1);

    let nic = NewIncomingConnection {
        server_address: peer,
        system_addresses: mcrs_raknet::SYSTEM_ADDRESSES,
        ping_time: 1000,
        pong_time: 1001,
    };
    let mut nic_buf = [0u8; 256];
    let nic_len = nic.write_to(&mut nic_buf).unwrap();

    let mut dg = Datagram::new(Uint24::new(0));
    dg.push_frame(Frame::new(
        Reliability::ReliableOrdered,
        Bytes::copy_from_slice(&nic_buf[..nic_len]),
    ));
    let mut dg_buf = [0u8; 512];
    let dg_len = dg.write_to(&mut dg_buf).unwrap();

    server.accept_inbound(peer, &dg_buf[..dg_len], 1002);

    let event = server.poll_event().unwrap();
    match event {
        RaknetEvent::Connected { client_guid, .. } => assert_eq!(client_guid, 0x9999),
        _ => panic!("expected Connected event"),
    }

    let spoofed_ocr2 = OpenConnectionRequest2 {
        server_address: peer,
        mtu_size: 1400,
        client_guid: 0xdead_beef,
    };
    let mut spoof_buf = [0u8; 128];
    let spoof_len = spoofed_ocr2.write_to(&mut spoof_buf).unwrap();

    server.accept_inbound(peer, &spoof_buf[..spoof_len], 1003);
    assert_eq!(server.session_count(), 1);
    assert!(server.poll_event().is_none());
}

#[test]
fn max_sessions_limit() {
    let mut server = RaknetServer::new(0x1234, Motd::default());

    for i in 0..MAX_SESSIONS {
        let port = (10000 + i) as u16;
        let peer: SocketAddr = SocketAddr::new(
            core::net::IpAddr::V4(core::net::Ipv4Addr::new(127, 0, 0, 1)),
            port,
        );
        let ocr2 = OpenConnectionRequest2 {
            server_address: peer,
            mtu_size: 1400,
            client_guid: i as u64,
        };
        let mut buf = [0u8; 128];
        let len = ocr2.write_to(&mut buf).unwrap();
        server.accept_inbound(peer, &buf[..len], 1000);
    }
    assert_eq!(server.session_count(), MAX_SESSIONS);

    let overflow_peer: SocketAddr = "127.0.0.1:40000".parse().unwrap();
    let ocr2 = OpenConnectionRequest2 {
        server_address: overflow_peer,
        mtu_size: 1400,
        client_guid: 0xbeef,
    };
    let mut buf = [0u8; 128];
    let len = ocr2.write_to(&mut buf).unwrap();
    server.accept_inbound(overflow_peer, &buf[..len], 1001);
    assert_eq!(server.session_count(), MAX_SESSIONS);
}

#[test]
fn connected_ping_no_overflow() {
    let ping = ConnectedPing {
        client_time: i64::MAX,
    };
    let mut buf = [0u8; 16];
    let len = ping.write_to(&mut buf).unwrap();

    let decoded = ConnectedPing::read_from(&buf[..len]).unwrap();
    assert_eq!(decoded.client_time, i64::MAX);

    let pong = ConnectedPong {
        client_time: decoded.client_time,
        server_time: 123456789,
    };
    let mut pong_buf = [0u8; 32];
    let pong_len = pong.write_to(&mut pong_buf).unwrap();
    let decoded_pong = ConnectedPong::read_from(&pong_buf[..pong_len]).unwrap();
    assert_eq!(decoded_pong.client_time, i64::MAX);
    assert_eq!(decoded_pong.server_time, 123456789);
}

#[test]
fn steady_path_zero_allocations() {
    let peer: SocketAddr = "127.0.0.1:19132".parse().unwrap();
    let mut session = RaknetSession::new(peer, 1400, 1, 1000).unwrap();

    let mut dg = Datagram::new(Uint24::new(0));
    dg.push_frame(Frame::new(
        Reliability::Unreliable,
        Bytes::from_static(b"warmup"),
    ));
    let mut buf = [0u8; 128];
    let len = dg.write_to(&mut buf).unwrap();
    let mut rcv = Vec::with_capacity(16);
    session.handle_packet(&buf[..len], 1000, &mut rcv);
    rcv.clear();

    let mut tx_outbound = alloc::collections::VecDeque::with_capacity(16);
    session.produce_outgoing_packets(1001, &mut tx_outbound);
    tx_outbound.clear();

    let before = mcrs_test_alloc::allocations();

    let num = Uint24::new(12345);
    let mut write_buf = [0u8; 3];
    num.write_to(&mut write_buf).unwrap();
    let (read_num, _) = Uint24::read_from(&write_buf).unwrap();
    assert_eq!(read_num, num);

    let after = mcrs_test_alloc::allocations();
    assert_eq!(after - before, 0);
}
