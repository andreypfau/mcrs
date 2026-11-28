use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32};

#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x05)]
pub struct Disconnect<'a> {
    pub reason: DisconnectReason,
    pub screen: DisconnectScreen<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisconnectScreen<'a> {
    Hidden,
    Shown {
        message: &'a str,
        filtered_message: &'a str,
    },
}

impl BedrockEncode for DisconnectScreen<'_> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Hidden => true.encode(w),
            Self::Shown {
                message,
                filtered_message,
            } => {
                false.encode(w)?;
                message.encode(w)?;
                filtered_message.encode(w)
            }
        }
    }
}

impl<'a> BedrockDecode<'a> for DisconnectScreen<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        if bool::decode(r)? {
            return Ok(Self::Hidden);
        }
        Ok(Self::Shown {
            message: BedrockDecode::decode(r)?,
            filtered_message: BedrockDecode::decode(r)?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisconnectReason(pub i32);

impl DisconnectReason {
    pub const UNKNOWN: Self = Self(0);
    pub const SERVER_FULL: Self = Self(25);
    pub const SERVER_OUTDATED: Self = Self(34);
    pub const CLIENT_OUTDATED: Self = Self(35);
    pub const DISCONNECTED: Self = Self(41);
    pub const INVALID_PLAYER: Self = Self(42);
    pub const LOGGED_IN_ELSEWHERE: Self = Self(43);
    pub const NOT_ALLOWED: Self = Self(45);
    pub const NOT_AUTHENTICATED: Self = Self(46);
    pub const UNKNOWN_PACKET: Self = Self(48);
    pub const UNEXPECTED_PACKET: Self = Self(49);
    pub const KICKED: Self = Self(55);
    pub const KICKED_FOR_IDLE: Self = Self(57);
    pub const RESOURCE_PACK_PROBLEM: Self = Self(58);
    pub const SHUTDOWN: Self = Self(68);
}

impl BedrockEncode for DisconnectReason {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        VarI32(self.0).encode(w)
    }
}

impl<'a> BedrockDecode<'a> for DisconnectReason {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarI32(reason) = VarI32::decode(r)?;
        Ok(Self(reason))
    }
}
