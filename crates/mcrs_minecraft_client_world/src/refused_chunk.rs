use std::fmt;

use mcrs_minecraft_protocol::Error;
use mcrs_minecraft_protocol_game::ColumnPos;

use crate::column_decode_error::ColumnDecodeError;

#[derive(Debug)]
pub(crate) enum RefusedChunk {
    Packet(Error),
    TrailingBytes(usize),
    Column {
        pos: ColumnPos,
        error: ColumnDecodeError,
    },
}

impl fmt::Display for RefusedChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusedChunk::Packet(error) => write!(f, "chunk packet: {error:?}"),
            RefusedChunk::TrailingBytes(left) => write!(f, "chunk packet: {left} bytes left over"),
            RefusedChunk::Column { pos, error } => write!(f, "column {pos:?}: {error}"),
        }
    }
}
