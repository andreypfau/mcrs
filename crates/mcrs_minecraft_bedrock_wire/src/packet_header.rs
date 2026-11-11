use alloc::vec::Vec;

use crate::error::Error;
use crate::traits::{BedrockDecode, BedrockEncode};
use crate::var_int::VarU32;

const ID_MASK: u32 = 0x3FF;
const SUB_CLIENT_MASK: u32 = 0x3;
const SENDER_SHIFT: u32 = 10;
const TARGET_SHIFT: u32 = 12;

/// The varuint32 in front of each game packet: the packet id in bits 0 to 9, the sender
/// sub-client in bits 10 and 11 and the target sub-client in bits 12 and 13.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PacketHeader {
    /// The packet id, below 1024.
    pub id: u16,
    /// The split-screen sub-client that sends the packet, below 4.
    pub sender_sub_client: u8,
    /// The split-screen sub-client the packet is for, below 4.
    pub target_sub_client: u8,
}

impl BedrockEncode for PacketHeader {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        let id = u32::from(self.id);
        let sender = u32::from(self.sender_sub_client);
        let target = u32::from(self.target_sub_client);
        if id > ID_MASK || sender > SUB_CLIENT_MASK || target > SUB_CLIENT_MASK {
            return Err(Error::PacketHeaderOverflow);
        }
        VarU32(id | sender << SENDER_SHIFT | target << TARGET_SHIFT).encode(w)
    }
}

impl<'a> BedrockDecode<'a> for PacketHeader {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarU32(raw) = VarU32::decode(r)?;
        Ok(Self {
            id: (raw & ID_MASK) as u16,
            sender_sub_client: (raw >> SENDER_SHIFT & SUB_CLIENT_MASK) as u8,
            target_sub_client: (raw >> TARGET_SHIFT & SUB_CLIENT_MASK) as u8,
        })
    }
}
