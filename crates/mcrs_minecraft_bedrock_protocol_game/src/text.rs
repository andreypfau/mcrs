//! Bedrock Edition Text packet.

use mcrs_minecraft_bedrock_protocol::{read_packet_header, write_packet_header};
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

use crate::packet_ids;

/// The type of a text message with the fields that the type carries.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextContent {
    Raw {
        message: String,
    },
    Chat {
        source_name: String,
        message: String,
    },
    Translate {
        message: String,
        parameters: Vec<String>,
    },
    Popup {
        message: String,
        parameters: Vec<String>,
    },
    JukeboxPopup {
        message: String,
        parameters: Vec<String>,
    },
    Tip {
        message: String,
    },
    System {
        message: String,
    },
    Whisper {
        source_name: String,
        message: String,
    },
    Announcement {
        source_name: String,
        message: String,
    },
    TextObject {
        message: String,
    },
    ObjectWhisper {
        message: String,
    },
}

impl TextContent {
    const fn id(&self) -> u8 {
        match self {
            Self::Raw { .. } => 0,
            Self::Chat { .. } => 1,
            Self::Translate { .. } => 2,
            Self::Popup { .. } => 3,
            Self::JukeboxPopup { .. } => 4,
            Self::Tip { .. } => 5,
            Self::System { .. } => 6,
            Self::Whisper { .. } => 7,
            Self::Announcement { .. } => 8,
            Self::TextObject { .. } => 9,
            Self::ObjectWhisper { .. } => 10,
        }
    }
}

/// A chat message, a system notice or a raw text sent to or from a client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextPacket {
    pub content: TextContent,
    pub needs_translation: bool,
    pub xuid: String,
    pub platform_chat_id: String,
}

impl BedrockEncode for TextPacket {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::TEXT, w)?;
        self.content.id().encode(w)?;
        self.needs_translation.encode(w)?;
        match &self.content {
            TextContent::Chat {
                source_name,
                message,
            }
            | TextContent::Whisper {
                source_name,
                message,
            }
            | TextContent::Announcement {
                source_name,
                message,
            } => {
                source_name.encode(w)?;
                message.encode(w)?;
            }
            TextContent::Raw { message }
            | TextContent::Tip { message }
            | TextContent::System { message }
            | TextContent::TextObject { message }
            | TextContent::ObjectWhisper { message } => message.encode(w)?,
            TextContent::Translate {
                message,
                parameters,
            }
            | TextContent::Popup {
                message,
                parameters,
            }
            | TextContent::JukeboxPopup {
                message,
                parameters,
            } => {
                message.encode(w)?;
                parameters.encode(w)?;
            }
        }
        self.xuid.encode(w)?;
        self.platform_chat_id.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for TextPacket {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::TEXT)?;
        let id = u8::decode(r)?;
        let needs_translation = bool::decode(r)?;
        let content = match id {
            0 => TextContent::Raw {
                message: String::decode(r)?,
            },
            1 => TextContent::Chat {
                source_name: String::decode(r)?,
                message: String::decode(r)?,
            },
            2 => TextContent::Translate {
                message: String::decode(r)?,
                parameters: Vec::decode(r)?,
            },
            3 => TextContent::Popup {
                message: String::decode(r)?,
                parameters: Vec::decode(r)?,
            },
            4 => TextContent::JukeboxPopup {
                message: String::decode(r)?,
                parameters: Vec::decode(r)?,
            },
            5 => TextContent::Tip {
                message: String::decode(r)?,
            },
            6 => TextContent::System {
                message: String::decode(r)?,
            },
            7 => TextContent::Whisper {
                source_name: String::decode(r)?,
                message: String::decode(r)?,
            },
            8 => TextContent::Announcement {
                source_name: String::decode(r)?,
                message: String::decode(r)?,
            },
            9 => TextContent::TextObject {
                message: String::decode(r)?,
            },
            10 => TextContent::ObjectWhisper {
                message: String::decode(r)?,
            },
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        };
        Ok(Self {
            content,
            needs_translation,
            xuid: String::decode(r)?,
            platform_chat_id: String::decode(r)?,
        })
    }
}
