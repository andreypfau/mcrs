#[allow(dead_code)]
mod common;

use bytes::BytesMut;
use mcrs_minecraft_protocol::decode::PacketDecoder;
use mcrs_minecraft_protocol::frame::{decompress, split_frame};
use mcrs_minecraft_protocol::{CompressionThreshold, Decode, VarInt};

const GOLDEN: &str = include_str!("fixtures/frames_golden.txt");

fn frames() -> Vec<(&'static str, Vec<u8>)> {
    GOLDEN
        .lines()
        .map(|line| {
            let (label, hex) = line
                .split_once(' ')
                .unwrap_or_else(|| panic!("malformed line: {line}"));
            (label, common::hex(hex))
        })
        .collect()
}

fn body(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

fn threshold_and_size(label: &str) -> (i32, usize) {
    let (threshold, size) = label
        .split_once('_')
        .unwrap_or_else(|| panic!("label without a size: {label}"));
    let threshold = match threshold {
        "off" => -1,
        threshold => threshold
            .strip_prefix('t')
            .and_then(|digits| digits.parse().ok())
            .unwrap_or_else(|| panic!("label without a threshold: {label}")),
    };
    (
        threshold,
        size.parse()
            .unwrap_or_else(|_| panic!("label without a size: {label}")),
    )
}

fn single_frames() -> impl Iterator<Item = (&'static str, Vec<u8>)> {
    frames()
        .into_iter()
        .filter(|(label, _)| !label.starts_with("stream_"))
}

#[test]
fn every_frame_the_game_wrote_decodes_to_its_body() {
    let mut seen = 0;
    for (label, bytes) in single_frames() {
        let (threshold, size) = threshold_and_size(label);
        let threshold = CompressionThreshold(threshold);
        let expected = body(size);

        let mut buf = BytesMut::from(&bytes[..]);
        let frame = split_frame(&mut buf).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert!(buf.is_empty(), "{label}: {} bytes left over", buf.len());
        let decompressed =
            decompress(frame, threshold, &mut None).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(&decompressed[..], &expected[..], "{label}");

        let mut decoder = PacketDecoder::new();
        decoder.set_compression(threshold);
        decoder.queue_bytes(BytesMut::from(&bytes[..]));
        let packet = decoder
            .try_next_packet()
            .unwrap_or_else(|e| panic!("{label}: {e}"))
            .unwrap_or_else(|| panic!("{label}: no packet"));
        assert_eq!(packet.id, 0, "{label}");
        assert_eq!(&packet.body[..], &expected[1..], "{label}");
        seen += 1;
    }
    assert!(seen >= 8, "only {seen} single-frame labels");
}

fn stated_data_length(label: &str) -> i32 {
    let (_, bytes) = single_frames()
        .find(|(candidate, _)| *candidate == label)
        .unwrap_or_else(|| panic!("{label} is missing from the golden"));
    let frame = split_frame(&mut BytesMut::from(&bytes[..])).unwrap();
    VarInt::decode(&mut &frame[..]).unwrap().0
}

#[test]
fn the_golden_holds_a_frame_compressed_at_exactly_its_threshold() {
    for label in ["t256_256", "t1_1"] {
        let (_, size) = threshold_and_size(label);
        assert_eq!(stated_data_length(label), size as i32, "{label}");
    }
}

#[test]
fn the_game_leaves_a_body_below_the_threshold_uncompressed() {
    assert_eq!(stated_data_length("t256_255"), 0);
}

#[test]
fn a_stream_the_game_wrote_decodes_across_its_compression_switch() {
    let (_, bytes) = frames()
        .into_iter()
        .find(|(label, _)| *label == "stream_off_3_then_t256_300")
        .expect("the stream label is missing from the golden");

    let mut decoder = PacketDecoder::new();
    decoder.queue_bytes(BytesMut::from(&bytes[..]));

    let first = decoder.try_next_packet().unwrap().unwrap();
    assert_eq!(first.id, 0);
    assert_eq!(&first.body[..], &body(3)[1..]);

    decoder.set_compression(CompressionThreshold(256));
    let second = decoder.try_next_packet().unwrap().unwrap();
    assert_eq!(second.id, 0);
    assert_eq!(&second.body[..], &body(300)[1..]);

    assert!(decoder.try_next_packet().unwrap().is_none());
}
