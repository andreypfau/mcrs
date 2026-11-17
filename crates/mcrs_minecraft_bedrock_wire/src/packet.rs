use alloc::vec::Vec;

use crate::error::Error;
use crate::packet_header::PacketHeader;
use crate::traits::{BedrockDecode, BedrockEncode};

pub trait Packet {
    const ID: u16;
}

pub fn encode_packet<P: Packet + BedrockEncode>(packet: &P, w: &mut Vec<u8>) -> Result<(), Error> {
    PacketHeader {
        id: P::ID,
        sender_sub_client: 0,
        target_sub_client: 0,
    }
    .encode(w)?;
    packet.encode(w)
}

pub fn decode_packet<'a, P: Packet + BedrockDecode<'a>>(r: &mut &'a [u8]) -> Result<P, Error> {
    let header = PacketHeader::decode(r)?;
    let actual = header.id;
    if actual != P::ID {
        return Err(Error::UnexpectedPacketId {
            expected: P::ID,
            actual,
        });
    }
    let packet = P::decode(r)?;
    if !r.is_empty() {
        return Err(Error::TrailingBytes);
    }
    Ok(packet)
}
