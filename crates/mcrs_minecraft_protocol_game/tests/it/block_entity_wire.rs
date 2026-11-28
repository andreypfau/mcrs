use mcrs_minecraft_block::block_entity_type_keys::BlockEntityType;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_protocol_game::chunk::ChunkDataBlockEntity;
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockEntityData;
use mcrs_voxel::BlockPos;
use std::borrow::Cow;

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

fn chest_entry(data: NbtCompound) -> ChunkDataBlockEntity<'static> {
    ChunkDataBlockEntity {
        packed_xz: 0x2c,
        y: -24,
        kind: BlockEntityType::Chest,
        data: Cow::Owned(data),
    }
}

#[test]
fn a_chunk_entry_data_is_a_compound_or_the_end_tag() {
    let cases: [(&[u8], Option<ChunkDataBlockEntity>); 4] = [
        (
            &[0x2c, 0xff, 0xe8, 0x01, 0x00],
            Some(chest_entry(NbtCompound::new())),
        ),
        (
            &[0x2c, 0xff, 0xe8, 0x01, 0x0a, 0x00],
            Some(chest_entry(NbtCompound::new())),
        ),
        (&[0x2c, 0xff, 0xe8, 0x01, 0x08, 0x00, 0x00], None),
        (&[0x2c, 0xff, 0xe8, 0x01], None),
    ];
    for (bytes, expected) in cases {
        let mut r = bytes;
        let decoded = ChunkDataBlockEntity::decode(&mut r).ok();
        assert_eq!(decoded, expected, "{bytes:02x?}");
        if expected.is_some() {
            assert!(r.is_empty(), "the entry {bytes:02x?} is taken whole");
        }
    }
}

#[test]
fn a_chunk_entry_writes_the_end_tag_only_for_data_without_keys() {
    let mut keyed = NbtCompound::new();
    keyed.put_int("Delay", 20);
    let mut empty = Vec::new();
    chest_entry(NbtCompound::new())
        .encode(&mut empty)
        .expect("encode");
    assert_eq!(empty, [0x2c, 0xff, 0xe8, 0x01, 0x00]);
    let entry = chest_entry(keyed);
    let mut bytes = Vec::new();
    entry.encode(&mut bytes).expect("encode");
    assert_eq!(bytes.get(4), Some(&0x0a));
    let mut r = bytes.as_slice();
    assert_eq!(ChunkDataBlockEntity::decode(&mut r).ok(), Some(entry));
    assert!(r.is_empty(), "the entry is taken whole");
}
