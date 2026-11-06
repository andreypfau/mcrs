use mcrs_minecraft_protocol::{Encode, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundBlockEvent;
use mcrs_voxel::BlockPos;

/// The packed position, the two arguments as unsigned bytes, then the block as a VarInt.
#[test]
fn a_block_event_is_the_position_the_two_arguments_and_the_block() {
    let packet = ClientboundBlockEvent {
        pos: BlockPos::new(1, 2, 3),
        action: 1,
        param: 200,
        block: VarInt(300),
    };
    let mut buf = Vec::new();
    packet.encode(&mut buf).expect("encode");
    assert_eq!(
        buf,
        [
            0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x30, 0x02, 0x01, 0xC8, 0xAC, 0x02
        ]
    );
}
