use mcrs_minecraft_bedrock_wire::{BedrockDecode, VarU32};

use crate::protocol_error::ProtocolError;

/// The packets of one checked batch, in the order the receiver handles them. Each item is
/// one packet: its header, then its payload.
#[derive(Clone, Debug)]
#[must_use]
pub struct PacketList<'a> {
    rest: &'a [u8],
}

impl<'a> PacketList<'a> {
    pub(crate) const EMPTY: Self = Self { rest: &[] };

    /// Checks each entry of `packet_list` before any packet is handed out.
    pub(crate) fn checked(packet_list: &'a [u8]) -> Result<Self, ProtocolError> {
        if packet_list.is_empty() {
            return Err(ProtocolError::EmptyPacketList);
        }
        let mut rest = packet_list;
        while !rest.is_empty() {
            let entry = split_entry(&mut rest)?;
            if entry.is_empty() {
                return Err(ProtocolError::EmptyEntry);
            }
        }
        Ok(Self { rest: packet_list })
    }
}

impl<'a> Iterator for PacketList<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        split_entry(&mut self.rest).ok()
    }
}

fn split_entry<'a>(rest: &mut &'a [u8]) -> Result<&'a [u8], ProtocolError> {
    let VarU32(len) = VarU32::decode(rest)?;
    let len = usize::try_from(len).map_err(|_| ProtocolError::TruncatedEntry)?;
    let Some((entry, tail)) = rest.split_at_checked(len) else {
        return Err(ProtocolError::TruncatedEntry);
    };
    *rest = tail;
    Ok(entry)
}
