use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, Packet};

/// A chat line, a translation, a tip or another text that the client shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text<'a> {
    /// Whether the client translates the message and the parameters.
    pub needs_translation: bool,
    /// The type with the fields that it carries.
    pub content: TextContent<'a>,
    /// The Xbox user id of the sender; empty for the server.
    pub sender_xuid: &'a str,
    /// Empty except on some consoles.
    pub platform_chat_id: &'a str,
    /// The message after the text filter; a server of mcrs writes an empty string, and the client
    /// then shows the message.
    pub filtered_message: &'a str,
}

/// The type of [`Text`]: the type byte selects the source name and the parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextContent<'a> {
    /// Type 0: raw text.
    Raw {
        /// The message.
        message: &'a str,
    },
    /// Type 1: chat, the one type that a client sends.
    Chat {
        /// The name of the sender.
        source_name: &'a str,
        /// The message.
        message: &'a str,
    },
    /// Type 2: a translation.
    Translation {
        /// The translation key.
        message: &'a str,
        /// The arguments of the translation key.
        parameters: Vec<&'a str>,
    },
    /// Type 3: a pop-up.
    Popup {
        /// The message.
        message: &'a str,
        /// The arguments of the translation key.
        parameters: Vec<&'a str>,
    },
    /// Type 4: a jukebox pop-up.
    JukeboxPopup {
        /// The message.
        message: &'a str,
        /// The arguments of the translation key.
        parameters: Vec<&'a str>,
    },
    /// Type 5: a tip.
    Tip {
        /// The message.
        message: &'a str,
    },
    /// Type 6: a system message.
    System {
        /// The message.
        message: &'a str,
    },
    /// Type 7: a whisper.
    Whisper {
        /// The name of the sender.
        source_name: &'a str,
        /// The message.
        message: &'a str,
    },
    /// Type 8: an announcement.
    Announcement {
        /// The name of the sender.
        source_name: &'a str,
        /// The message.
        message: &'a str,
    },
    /// Type 9: a whisper as a JSON text object with the key `rawtext`.
    JsonWhisper {
        /// The JSON text object.
        message: &'a str,
    },
    /// Type 10: a JSON text object with the key `rawtext`.
    Json {
        /// The JSON text object.
        message: &'a str,
    },
    /// Type 11: an announcement as a JSON text object with the key `rawtext`.
    JsonAnnouncement {
        /// The JSON text object.
        message: &'a str,
    },
}

impl TextContent<'_> {
    const fn wire_type(&self) -> u8 {
        match self {
            Self::Raw { .. } => 0,
            Self::Chat { .. } => 1,
            Self::Translation { .. } => 2,
            Self::Popup { .. } => 3,
            Self::JukeboxPopup { .. } => 4,
            Self::Tip { .. } => 5,
            Self::System { .. } => 6,
            Self::Whisper { .. } => 7,
            Self::Announcement { .. } => 8,
            Self::JsonWhisper { .. } => 9,
            Self::Json { .. } => 10,
            Self::JsonAnnouncement { .. } => 11,
        }
    }

    fn encode_fields(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        use TextContent::*;
        match self {
            Raw { message }
            | Tip { message }
            | System { message }
            | JsonWhisper { message }
            | Json { message }
            | JsonAnnouncement { message } => message.encode(w),
            Chat {
                source_name,
                message,
            }
            | Whisper {
                source_name,
                message,
            }
            | Announcement {
                source_name,
                message,
            } => {
                source_name.encode(w)?;
                message.encode(w)
            }
            Translation {
                message,
                parameters,
            }
            | Popup {
                message,
                parameters,
            }
            | JukeboxPopup {
                message,
                parameters,
            } => {
                message.encode(w)?;
                parameters.as_slice().encode(w)
            }
        }
    }
}

impl<'a> TextContent<'a> {
    fn decode_fields(wire_type: u8, r: &mut &'a [u8]) -> Result<Self, Error> {
        use TextContent::*;
        Ok(match wire_type {
            0 => Raw {
                message: BedrockDecode::decode(r)?,
            },
            1 => Chat {
                source_name: BedrockDecode::decode(r)?,
                message: BedrockDecode::decode(r)?,
            },
            2 => Translation {
                message: BedrockDecode::decode(r)?,
                parameters: BedrockDecode::decode(r)?,
            },
            3 => Popup {
                message: BedrockDecode::decode(r)?,
                parameters: BedrockDecode::decode(r)?,
            },
            4 => JukeboxPopup {
                message: BedrockDecode::decode(r)?,
                parameters: BedrockDecode::decode(r)?,
            },
            5 => Tip {
                message: BedrockDecode::decode(r)?,
            },
            6 => System {
                message: BedrockDecode::decode(r)?,
            },
            7 => Whisper {
                source_name: BedrockDecode::decode(r)?,
                message: BedrockDecode::decode(r)?,
            },
            8 => Announcement {
                source_name: BedrockDecode::decode(r)?,
                message: BedrockDecode::decode(r)?,
            },
            9 => JsonWhisper {
                message: BedrockDecode::decode(r)?,
            },
            10 => Json {
                message: BedrockDecode::decode(r)?,
            },
            11 => JsonAnnouncement {
                message: BedrockDecode::decode(r)?,
            },
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        })
    }
}

impl Packet for Text<'_> {
    const ID: u16 = 0x09;
}

impl BedrockEncode for Text<'_> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.content.wire_type().encode(w)?;
        self.needs_translation.encode(w)?;
        self.content.encode_fields(w)?;
        self.sender_xuid.encode(w)?;
        self.platform_chat_id.encode(w)?;
        self.filtered_message.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for Text<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let wire_type = u8::decode(r)?;
        let needs_translation = bool::decode(r)?;
        Ok(Self {
            needs_translation,
            content: TextContent::decode_fields(wire_type, r)?,
            sender_xuid: BedrockDecode::decode(r)?,
            platform_chat_id: BedrockDecode::decode(r)?,
            filtered_message: BedrockDecode::decode(r)?,
        })
    }
}
