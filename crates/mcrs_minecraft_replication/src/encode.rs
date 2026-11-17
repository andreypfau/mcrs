use bytes::{Bytes, BytesMut};
use mcrs_minecraft_protocol::{Packet, Result, write_record};
use mcrs_minecraft_wire::Encode;

pub fn record<P: Packet + Encode>(packet: &P) -> Result<Bytes> {
    let mut bytes = BytesMut::new();
    write_record(&mut bytes, packet)?;
    Ok(bytes.freeze())
}
