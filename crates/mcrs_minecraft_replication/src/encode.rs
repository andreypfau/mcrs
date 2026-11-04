use bytes::{BufMut, Bytes, BytesMut};
use mcrs_minecraft_protocol::{ErrorKind, Packet, Result};
use mcrs_minecraft_wire::Encode;

const LENGTH_BYTES: usize = size_of::<u32>();

/// `packet` as a record: its length as a big-endian `u32`, then its id and its body. Records
/// follow each other in a buffer with no gap, and the writer of a connection frames each one.
pub fn record<P: Packet + Encode>(packet: &P) -> Result<Bytes> {
    let mut bytes = BytesMut::new();
    bytes.put_u32(0);
    packet.encode_with_id((&mut bytes).writer())?;
    let body = bytes.len().saturating_sub(LENGTH_BYTES);
    let length = u32::try_from(body).map_err(|_| ErrorKind::TooLong {
        len: body,
        max: u32::MAX as usize,
    })?;
    if let Some(prefix) = bytes.get_mut(..LENGTH_BYTES) {
        prefix.copy_from_slice(&length.to_be_bytes());
    }
    Ok(bytes.freeze())
}
