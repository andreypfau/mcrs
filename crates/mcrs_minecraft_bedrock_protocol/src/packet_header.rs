use mcrs_minecraft_bedrock_wire::{BedrockEncode, Error, VarU32};

// The two sub-client ids sit above bit 10 of the header.
const PACKET_ID_MASK: u32 = 0x3FF;

/// Writes the header of a packet with the given id and no sub-client.
pub fn write_packet_header(packet_id: u32, w: &mut Vec<u8>) -> Result<(), Error> {
    VarU32(packet_id).encode(w)
}

/// Reads a packet header and checks that it carries `expected_id`.
pub fn read_packet_header(r: &mut &[u8], expected_id: u32) -> Result<(), Error> {
    let actual = VarU32::decode(r)?.0 & PACKET_ID_MASK;
    if actual != expected_id {
        return Err(Error::UnexpectedPacketId {
            expected: expected_id,
            actual,
        });
    }
    Ok(())
}

pub(crate) fn encode_be_i32(value: i32, w: &mut Vec<u8>) {
    w.extend_from_slice(&value.to_be_bytes());
}

pub(crate) fn decode_be_i32(r: &mut &[u8]) -> Result<i32, Error> {
    let Some((bytes, rest)) = r.split_first_chunk::<4>() else {
        return Err(Error::UnexpectedEof);
    };
    *r = rest;
    Ok(i32::from_be_bytes(*bytes))
}
