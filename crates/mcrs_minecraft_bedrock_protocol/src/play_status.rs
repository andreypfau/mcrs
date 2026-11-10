use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

use crate::packet_header::{decode_be_i32, encode_be_i32, read_packet_header, write_packet_header};
use crate::packet_ids;

/// A step of the login and spawn flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[repr(i32)]
pub enum PlayStatusType {
    LoginSuccess = 0,
    FailedClient = 1,
    FailedServer = 2,
    PlayerSpawn = 3,
    InvalidTenant = 4,
    VanillaEdu = 5,
    EduVanilla = 6,
    ServerFull = 7,
}

/// The server reports a step of the login and spawn flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayStatus {
    pub status: PlayStatusType,
}

impl BedrockEncode for PlayStatus {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::PLAY_STATUS, w)?;
        encode_be_i32(self.status as i32, w);
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for PlayStatus {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::PLAY_STATUS)?;
        let status = match decode_be_i32(r)? {
            0 => PlayStatusType::LoginSuccess,
            1 => PlayStatusType::FailedClient,
            2 => PlayStatusType::FailedServer,
            3 => PlayStatusType::PlayerSpawn,
            4 => PlayStatusType::InvalidTenant,
            5 => PlayStatusType::VanillaEdu,
            6 => PlayStatusType::EduVanilla,
            7 => PlayStatusType::ServerFull,
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        };
        Ok(Self { status })
    }
}
