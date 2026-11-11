use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarU32};

const PACKET_ID_MASK: u32 = 0x3FF;

pub(crate) fn write_packet_header(packet_id: u32, w: &mut Vec<u8>) -> Result<(), Error> {
    VarU32(packet_id).encode(w)
}

pub(crate) fn read_packet_header(r: &mut &[u8], expected_id: u32) -> Result<(), Error> {
    let actual = VarU32::decode(r)?.0 & PACKET_ID_MASK;
    if actual != expected_id {
        return Err(Error::UnexpectedPacketId {
            expected: expected_id,
            actual,
        });
    }
    Ok(())
}
