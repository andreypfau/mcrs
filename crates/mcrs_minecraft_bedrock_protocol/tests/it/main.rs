use std::io::Write;

use flate2::write::DeflateEncoder;
use mcrs_minecraft_bedrock_protocol::{
    BatchCompression, CompressionAlgorithm, Disconnect, Experiment, Login, MINECRAFT_VERSION,
    NetworkSettings, PROTOCOL_VERSION, PlayStatus, PlayStatusType, ProtocolError,
    RequestNetworkSettings, ResourcePackClientResponse, ResourcePackInfoEntry,
    ResourcePackResponseStatus, ResourcePackStack, ResourcePackStackEntry, ResourcePacksInfo,
    decode_batch, encode_batch,
};
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32};

#[test]
fn request_network_settings_round_trip() {
    let packet = RequestNetworkSettings {
        client_protocol_version: PROTOCOL_VERSION,
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = RequestNetworkSettings::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}

#[test]
fn network_settings_round_trip() {
    let packet = NetworkSettings {
        compression_threshold: 512,
        compression_algorithm: CompressionAlgorithm::Deflate,
        client_throttle: true,
        client_throttle_threshold: 30,
        client_throttle_scalar: 1.5,
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = NetworkSettings::decode(&mut r).unwrap();
    assert_eq!(decoded.compression_threshold, packet.compression_threshold);
    assert_eq!(decoded.compression_algorithm, packet.compression_algorithm);
    assert_eq!(decoded.client_throttle, packet.client_throttle);
    assert_eq!(
        decoded.client_throttle_threshold,
        packet.client_throttle_threshold
    );
    assert_eq!(
        decoded.client_throttle_scalar,
        packet.client_throttle_scalar
    );
    assert!(r.is_empty());
}

#[test]
fn login_round_trip() {
    let packet = Login {
        protocol_version: PROTOCOL_VERSION,
        payload: vec![1, 2, 3, 4, 5, 6, 7, 8],
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = Login::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}

#[test]
fn play_status_round_trip() {
    let packet = PlayStatus {
        status: PlayStatusType::LoginSuccess,
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = PlayStatus::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}

#[test]
fn disconnect_round_trip() {
    let packet = Disconnect {
        reason: VarI32(42),
        hide_disconnect_screen: false,
        message: Some("Server closed".to_string()),
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded = Disconnect::decode(&mut r).unwrap();
    assert_eq!(decoded, packet);
    assert!(r.is_empty());
}

#[test]
fn resource_packs_negotiation_round_trip() {
    let info = ResourcePacksInfo {
        must_accept: false,
        has_scripts: false,
        force_server_packs: false,
        behavior_packs: vec![],
        resource_packs: vec![ResourcePackInfoEntry {
            uuid: "123e4567-e89b-12d3-a456-426614174000".to_string(),
            version: "1.0.0".to_string(),
            size: 1024,
            content_key: "".to_string(),
            sub_pack_name: "".to_string(),
            content_identity: "".to_string(),
            has_scripts: false,
        }],
    };
    let mut buf = Vec::new();
    info.encode(&mut buf).unwrap();

    let mut r = buf.as_slice();
    let decoded_info = ResourcePacksInfo::decode(&mut r).unwrap();
    assert_eq!(decoded_info, info);

    let stack = ResourcePackStack {
        must_accept: false,
        behavior_packs: vec![],
        resource_packs: vec![ResourcePackStackEntry {
            uuid: "123e4567-e89b-12d3-a456-426614174000".to_string(),
            version: "1.0.0".to_string(),
            sub_pack_name: "".to_string(),
        }],
        game_version: MINECRAFT_VERSION.to_string(),
        experiments: vec![Experiment {
            name: "data_driven_items".to_string(),
            enabled: true,
        }],
        were_experiments_previously_enabled: true,
    };
    let mut stack_buf = Vec::new();
    stack.encode(&mut stack_buf).unwrap();

    let mut r = stack_buf.as_slice();
    let decoded_stack = ResourcePackStack::decode(&mut r).unwrap();
    assert_eq!(decoded_stack, stack);

    let client_response = ResourcePackClientResponse {
        response_status: ResourcePackResponseStatus::Completed,
        pack_ids: vec!["123e4567-e89b-12d3-a456-426614174000".to_string()],
    };
    let mut response_buf = Vec::new();
    client_response.encode(&mut response_buf).unwrap();

    let mut r = response_buf.as_slice();
    let decoded_response = ResourcePackClientResponse::decode(&mut r).unwrap();
    assert_eq!(decoded_response, client_response);
}

#[test]
fn batch_packet_compression_round_trip() {
    let mut p1 = Vec::new();
    PlayStatus {
        status: PlayStatusType::LoginSuccess,
    }
    .encode(&mut p1)
    .unwrap();

    let mut p2 = Vec::new();
    Disconnect {
        reason: VarI32(0),
        hide_disconnect_screen: true,
        message: None,
    }
    .encode(&mut p2)
    .unwrap();

    let raw_packets = [p1.as_slice(), p2.as_slice()];
    let deflate = BatchCompression::Negotiated(CompressionAlgorithm::Deflate);
    let batch_bytes = encode_batch(&raw_packets, deflate, 0).unwrap();
    assert_eq!(batch_bytes[..2], [0xFE, 0x00]);

    let decoded_packets = decode_batch(&batch_bytes, deflate, 1 << 20).unwrap();
    assert_eq!(decoded_packets.len(), 2);
    assert_eq!(decoded_packets[0], p1);
    assert_eq!(decoded_packets[1], p2);
}

#[test]
fn request_network_settings_bytes() {
    let packet = RequestNetworkSettings {
        client_protocol_version: PROTOCOL_VERSION,
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).unwrap();
    assert_eq!(buf, [0xC1, 0x01, 0x00, 0x00, 0x02, 0xD9]);
}

#[test]
fn batch_below_threshold_carries_no_compression_marker() {
    let deflate = BatchCompression::Negotiated(CompressionAlgorithm::Deflate);
    let batch = encode_batch(&[&[0x01, 0x02]], deflate, 1024).unwrap();
    assert_eq!(batch, [0xFE, 0xFF, 0x02, 0x01, 0x02]);
    assert_eq!(
        decode_batch(&batch, deflate, 1024).unwrap(),
        [vec![0x01, 0x02]]
    );
}

#[test]
fn pending_batch_has_no_marker() {
    let batch = encode_batch(&[&[0x07]], BatchCompression::Pending, 0).unwrap();
    assert_eq!(batch, [0xFE, 0x01, 0x07]);
    assert_eq!(
        decode_batch(&batch, BatchCompression::Pending, 0).unwrap(),
        [vec![0x07]]
    );
}

#[test]
fn inflated_batch_past_limit_is_rejected() {
    let mut encoder = DeflateEncoder::new(vec![0xFE, 0x00], flate2::Compression::best());
    encoder.write_all(&[0; 1 << 16]).unwrap();
    let batch = encoder.finish().unwrap();
    let deflate = BatchCompression::Negotiated(CompressionAlgorithm::Deflate);
    assert!(matches!(
        decode_batch(&batch, deflate, 1024),
        Err(ProtocolError::BatchTooLarge { limit: 1024 })
    ));
}

#[test]
fn corrupt_deflate_batch_is_an_error() {
    let deflate = BatchCompression::Negotiated(CompressionAlgorithm::Deflate);
    let batch = [0xFE, 0x00, 0xFF, 0xFF, 0xFF];
    assert!(matches!(
        decode_batch(&batch, deflate, 1024),
        Err(ProtocolError::Deflate(_))
    ));
}

#[test]
fn snappy_batch_is_unsupported() {
    let snappy = BatchCompression::Negotiated(CompressionAlgorithm::Snappy);
    assert!(matches!(
        encode_batch(&[&[0x01]], snappy, 0),
        Err(ProtocolError::UnsupportedCompression(
            CompressionAlgorithm::Snappy
        ))
    ));
}

#[test]
fn unknown_compression_algorithm_is_rejected() {
    let mut r: &[u8] = &[0x07, 0x00];
    assert_eq!(
        CompressionAlgorithm::decode(&mut r),
        Err(mcrs_minecraft_bedrock_wire::Error::UnknownEnumValue(7))
    );
}
