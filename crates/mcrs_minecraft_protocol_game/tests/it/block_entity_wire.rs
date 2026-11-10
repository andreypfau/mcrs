use mcrs_minecraft_block::block_entity_type_keys::BlockEntityType;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockEntityData;
use mcrs_voxel::BlockPos;

fn frame(pos: BlockPos, body: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut buf = Vec::new();
    pos.encode(&mut buf).expect("encode");
    body(&mut buf);
    buf
}

fn decoded(bytes: &[u8]) -> ClientboundBlockEntityData {
    let mut r = bytes;
    let packet = ClientboundBlockEntityData::decode(&mut r).expect("a frame never fails to decode");
    assert!(r.is_empty(), "the whole frame is taken");
    packet
}

#[test]
fn a_sign_update_round_trips() {
    let pos = BlockPos::new(1, 2, 3);
    let packet = ClientboundBlockEntityData {
        pos,
        data: Some((BlockEntityType::Sign, NbtCompound::new())),
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).expect("encode");
    assert_eq!(decoded(&buf), packet);
}

/// A type id past the registry and NBT that does not parse both reach the client as an update
/// without data, never as a frame that ends the connection.
#[test]
fn a_frame_the_client_cannot_read_decodes_without_data() {
    let pos = BlockPos::new(4, 5, 6);
    let unknown_type = frame(pos, |buf| {
        VarInt(100_000).encode(&mut *buf).expect("encode");
        NbtCompound::new().encode(&mut *buf).expect("encode");
    });
    let bad_nbt = frame(pos, |buf| {
        BlockEntityType::Sign.encode(&mut *buf).expect("encode");
        buf.extend_from_slice(&[0x0A, 0xFF, 0xFF, 0xFF]);
    });
    let truncated = frame(pos, |_| {});
    for bytes in [unknown_type, bad_nbt, truncated] {
        assert_eq!(
            decoded(&bytes),
            ClientboundBlockEntityData { pos, data: None }
        );
    }
}
