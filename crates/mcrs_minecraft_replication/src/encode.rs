use bytes::{Bytes, BytesMut};
use mcrs_minecraft_protocol::{Packet, Result, write_record};
use mcrs_minecraft_wire::Encode;

/// `packet` as a record: its length as a big-endian `u32`, then its id and its body. Records
/// follow each other in a buffer with no gap, and the writer of a connection frames each one.
pub fn record<P: Packet + Encode>(packet: &P) -> Result<Bytes> {
    let mut bytes = BytesMut::new();
    write_record(&mut bytes, packet)?;
    Ok(bytes.freeze())
}
