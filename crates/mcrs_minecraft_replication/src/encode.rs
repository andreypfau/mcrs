use bytes::Bytes;
use mcrs_minecraft_protocol::{Packet, Result};
use mcrs_minecraft_wire::Encode;

pub(crate) fn encode<P: Packet + Encode>(packet: &P) -> Result<Bytes> {
    let mut bytes = Vec::new();
    packet.encode_with_id(&mut bytes)?;
    Ok(Bytes::from(bytes))
}
