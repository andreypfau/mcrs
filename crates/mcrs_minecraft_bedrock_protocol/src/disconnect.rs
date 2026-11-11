use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32};

use crate::packet_header::{read_packet_header, write_packet_header};
use crate::packet_ids;

/// Disconnect packet sent to inform client of a disconnection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disconnect {
    pub reason: VarI32,
    pub hide_disconnect_screen: bool,
    pub message: Option<String>,
}

impl BedrockEncode for Disconnect {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::DISCONNECT, w)?;
        self.reason.encode(w)?;
        self.hide_disconnect_screen.encode(w)?;
        match &self.message {
            Some(message) => {
                true.encode(w)?;
                message.encode(w)?;
            }
            None => false.encode(w)?,
        }
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for Disconnect {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::DISCONNECT)?;
        let reason = VarI32::decode(r)?;
        let hide_disconnect_screen = bool::decode(r)?;
        let message = if bool::decode(r)? {
            Some(String::decode(r)?)
        } else {
            None
        };
        Ok(Self {
            reason,
            hide_disconnect_screen,
            message,
        })
    }
}
