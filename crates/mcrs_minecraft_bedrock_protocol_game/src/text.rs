use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, Packet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text<'a> {
    pub needs_translation: bool,
    pub content: TextContent<'a>,
    pub sender_xuid: &'a str,
    pub platform_chat_id: &'a str,
    pub filtered_message: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextContent<'a> {
    Raw {
        message: &'a str,
    },
    Chat {
        source_name: &'a str,
        message: &'a str,
    },
    Translation {
        message: &'a str,
        parameters: Vec<&'a str>,
    },
    Popup {
        message: &'a str,
        parameters: Vec<&'a str>,
    },
    JukeboxPopup {
        message: &'a str,
        parameters: Vec<&'a str>,
    },
    Tip {
        message: &'a str,
    },
    System {
        message: &'a str,
    },
    Whisper {
        source_name: &'a str,
        message: &'a str,
    },
    Announcement {
        source_name: &'a str,
        message: &'a str,
    },
    JsonWhisper {
        message: &'a str,
    },
    Json {
        message: &'a str,
    },
    JsonAnnouncement {
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
