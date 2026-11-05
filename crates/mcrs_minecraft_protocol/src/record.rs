use bytes::{BufMut, BytesMut};

use crate::{Encode, ErrorKind, Packet, Result};

/// The bytes of the length in front of each record.
pub const RECORD_LENGTH_BYTES: usize = size_of::<u32>();

/// Appends `packet` to `records` as a record: its length as a big-endian `u32`, then its id and
/// its body. Records follow each other with no gap, and the writer of a connection frames each
/// one. A packet that fails to encode leaves `records` as it was.
pub fn write_record<P: Packet + Encode>(records: &mut BytesMut, packet: &P) -> Result<()> {
    let start = records.len();
    records.put_u32(0);
    if let Err(error) = packet.encode_with_id(records.writer()) {
        records.truncate(start);
        return Err(error);
    }
    let body = records.len() - start - RECORD_LENGTH_BYTES;
    let Ok(length) = u32::try_from(body) else {
        records.truncate(start);
        return Err(ErrorKind::TooLong {
            len: body,
            max: u32::MAX as usize,
        }
        .into());
    };
    if let Some(prefix) = records.get_mut(start..start + RECORD_LENGTH_BYTES) {
        prefix.copy_from_slice(&length.to_be_bytes());
    }
    Ok(())
}
