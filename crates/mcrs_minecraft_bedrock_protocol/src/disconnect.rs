use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32};

/// Tells the client why the server closes the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x05)]
pub struct Disconnect<'a> {
    /// The reason.
    pub reason: DisconnectReason,
    /// Whether the client shows a disconnect screen, and its text.
    pub screen: DisconnectScreen<'a>,
}

/// The screen of [`Disconnect`]: the hide-screen flag selects whether the messages follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisconnectScreen<'a> {
    /// The client shows no disconnect screen.
    Hidden,
    /// The client shows a disconnect screen with a message.
    Shown {
        /// The text that the client shows.
        message: &'a str,
        /// The message after a text filter; a server of mcrs writes an empty string.
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

/// The reason of [`Disconnect`]. A receiver accepts each value; the constants are the
/// reasons that a server of mcrs sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DisconnectReason(pub i32);

impl DisconnectReason {
    /// The reason is not known.
    pub const UNKNOWN: Self = Self(0);
    /// The server is full.
    pub const SERVER_FULL: Self = Self(25);
    /// The server is older than the client.
    pub const SERVER_OUTDATED: Self = Self(34);
    /// The client is older than the server.
    pub const CLIENT_OUTDATED: Self = Self(35);
    /// Disconnected.
    pub const DISCONNECTED: Self = Self(41);
    /// The player is not valid.
    pub const INVALID_PLAYER: Self = Self(42);
    /// The player logged in from another location.
    pub const LOGGED_IN_ELSEWHERE: Self = Self(43);
    /// Not allowed.
    pub const NOT_ALLOWED: Self = Self(45);
    /// Not authenticated.
    pub const NOT_AUTHENTICATED: Self = Self(46);
    /// Unknown packet.
    pub const UNKNOWN_PACKET: Self = Self(48);
    /// Unexpected packet.
    pub const UNEXPECTED_PACKET: Self = Self(49);
    /// Kicked.
    pub const KICKED: Self = Self(55);
    /// Kicked because the player was idle.
    pub const KICKED_FOR_IDLE: Self = Self(57);
    /// A resource pack problem.
    pub const RESOURCE_PACK_PROBLEM: Self = Self(58);
    /// The server shuts down.
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
