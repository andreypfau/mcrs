extern crate alloc;

use alloc::string::String;
use core::net::SocketAddr;

use crate::magic::{RAKNET_MAGIC, is_valid_magic};
use crate::wire::AddressCodec;

pub mod id {
    pub const CONNECTED_PING: u8 = 0x00;
    pub const UNCONNECTED_PING: u8 = 0x01;
    pub const UNCONNECTED_PING_OPEN_CONNECTIONS: u8 = 0x02;
    pub const CONNECTED_PONG: u8 = 0x03;
    pub const OPEN_CONNECTION_REQUEST_1: u8 = 0x05;
    pub const OPEN_CONNECTION_REPLY_1: u8 = 0x06;
    pub const OPEN_CONNECTION_REQUEST_2: u8 = 0x07;
    pub const OPEN_CONNECTION_REPLY_2: u8 = 0x08;
    pub const CONNECTION_REQUEST: u8 = 0x09;
    pub const CONNECTION_REQUEST_ACCEPTED: u8 = 0x10;
    pub const NEW_INCOMING_CONNECTION: u8 = 0x13;
    pub const DISCONNECT_NOTIFICATION: u8 = 0x15;
    pub const INCOMPATIBLE_PROTOCOL_VERSION: u8 = 0x19;
    pub const UNCONNECTED_PONG: u8 = 0x1c;
    pub const FRAME_SET_MIN: u8 = 0x80;
    pub const FRAME_SET_MAX: u8 = 0x8d;
    pub const NACK: u8 = 0xa0;
    pub const ACK: u8 = 0xc0;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnconnectedPing {
    pub client_time: i64,
    pub client_guid: u64,
    pub open_connections: bool,
}

impl UnconnectedPing {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        let packet_id = *buf.first()?;
        let open_connections = match packet_id {
            id::UNCONNECTED_PING => false,
            id::UNCONNECTED_PING_OPEN_CONNECTIONS => true,
            _ => return None,
        };
        let time_slice = buf.get(1..9)?;
        let client_time = i64::from_be_bytes(time_slice.try_into().ok()?);
        let magic_slice = buf.get(9..25)?;
        if !is_valid_magic(magic_slice) {
            return None;
        }
        let guid_slice = buf.get(25..33)?;
        let client_guid = u64::from_be_bytes(guid_slice.try_into().ok()?);
        Some(Self {
            client_time,
            client_guid,
            open_connections,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let needed = 33;
        if buf.len() < needed {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = if self.open_connections {
                id::UNCONNECTED_PING_OPEN_CONNECTIONS
            } else {
                id::UNCONNECTED_PING
            };
        }
        let time_target = buf.get_mut(1..9)?;
        time_target.copy_from_slice(&self.client_time.to_be_bytes());
        let magic_target = buf.get_mut(9..25)?;
        magic_target.copy_from_slice(&RAKNET_MAGIC);
        let guid_target = buf.get_mut(25..33)?;
        guid_target.copy_from_slice(&self.client_guid.to_be_bytes());
        Some(needed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnconnectedPong {
    pub client_time: i64,
    pub server_guid: u64,
    pub motd: String,
}

impl UnconnectedPong {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::UNCONNECTED_PONG {
            return None;
        }
        let time_slice = buf.get(1..9)?;
        let guid_slice = buf.get(9..17)?;
        let client_time = i64::from_be_bytes(time_slice.try_into().ok()?);
        let server_guid = u64::from_be_bytes(guid_slice.try_into().ok()?);

        let magic_slice = buf.get(17..33)?;
        if !is_valid_magic(magic_slice) {
            return None;
        }
        let len_slice = buf.get(33..35)?;
        let motd_len = u16::from_be_bytes(len_slice.try_into().ok()?) as usize;
        let motd_bytes = buf.get(35..35 + motd_len)?;
        let motd = String::from_utf8(motd_bytes.to_vec()).ok()?;
        Some(Self {
            client_time,
            server_guid,
            motd,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let motd_bytes = self.motd.as_bytes();
        let needed = 35 + motd_bytes.len();
        if buf.len() < needed {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::UNCONNECTED_PONG;
        }
        let time_target = buf.get_mut(1..9)?;
        time_target.copy_from_slice(&self.client_time.to_be_bytes());
        let guid_target = buf.get_mut(9..17)?;
        guid_target.copy_from_slice(&self.server_guid.to_be_bytes());
        let magic_target = buf.get_mut(17..33)?;
        magic_target.copy_from_slice(&RAKNET_MAGIC);
        let len_target = buf.get_mut(33..35)?;
        len_target.copy_from_slice(&(motd_bytes.len() as u16).to_be_bytes());
        let motd_target = buf.get_mut(35..needed)?;
        motd_target.copy_from_slice(motd_bytes);
        Some(needed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Motd {
    pub edition: String,
    pub server_name: String,
    pub protocol_version: u32,
    pub version_name: String,
    pub player_count: u32,
    pub max_players: u32,
    pub server_guid: u64,
    pub world_name: String,
    pub game_mode: String,
    pub game_mode_numeric: u32,
    pub port_v4: u16,
    pub port_v6: u16,
}

impl Default for Motd {
    fn default() -> Self {
        Self {
            edition: String::from("MCPE"),
            server_name: String::from("mcrs Bedrock Server"),
            protocol_version: 729,
            version_name: String::from("1.21.40"),
            player_count: 0,
            max_players: 20,
            server_guid: 0,
            world_name: String::from("world"),
            game_mode: String::from("Survival"),
            game_mode_numeric: 1,
            port_v4: 19132,
            port_v6: 19133,
        }
    }
}

impl Motd {
    #[must_use]
    pub fn to_string_lossless(&self) -> String {
        alloc::format!(
            "{};{};{};{};{};{};{};{};{};{};{};{};",
            self.edition,
            self.server_name,
            self.protocol_version,
            self.version_name,
            self.player_count,
            self.max_players,
            self.server_guid,
            self.world_name,
            self.game_mode,
            self.game_mode_numeric,
            self.port_v4,
            self.port_v6
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenConnectionRequest1 {
    pub protocol_version: u8,
    pub mtu_size: u16,
}

impl OpenConnectionRequest1 {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::OPEN_CONNECTION_REQUEST_1 {
            return None;
        }
        let magic_slice = buf.get(1..17)?;
        if !is_valid_magic(magic_slice) {
            return None;
        }
        let protocol_version = *buf.get(17)?;
        let mtu_size = (buf.len() + 28) as u16;
        Some(Self {
            protocol_version,
            mtu_size,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let needed = self.mtu_size.saturating_sub(28) as usize;
        if buf.len() < needed || needed < 18 {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::OPEN_CONNECTION_REQUEST_1;
        }
        let magic_target = buf.get_mut(1..17)?;
        magic_target.copy_from_slice(&RAKNET_MAGIC);
        if let Some(slot) = buf.get_mut(17) {
            *slot = self.protocol_version;
        }
        if let Some(rest) = buf.get_mut(18..needed) {
            rest.fill(0);
        }
        Some(needed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenConnectionReply1 {
    pub server_guid: u64,
    pub security: bool,
    pub mtu_size: u16,
}

impl OpenConnectionReply1 {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::OPEN_CONNECTION_REPLY_1 {
            return None;
        }
        let magic_slice = buf.get(1..17)?;
        if !is_valid_magic(magic_slice) {
            return None;
        }
        let guid_slice = buf.get(17..25)?;
        let server_guid = u64::from_be_bytes(guid_slice.try_into().ok()?);
        let security = *buf.get(25)? != 0;
        let mtu_slice = buf.get(26..28)?;
        let mtu_size = u16::from_be_bytes(mtu_slice.try_into().ok()?);
        Some(Self {
            server_guid,
            security,
            mtu_size,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let needed = 28;
        if buf.len() < needed {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::OPEN_CONNECTION_REPLY_1;
        }
        let magic_target = buf.get_mut(1..17)?;
        magic_target.copy_from_slice(&RAKNET_MAGIC);
        let guid_target = buf.get_mut(17..25)?;
        guid_target.copy_from_slice(&self.server_guid.to_be_bytes());
        if let Some(slot) = buf.get_mut(25) {
            *slot = if self.security { 1 } else { 0 };
        }
        let mtu_target = buf.get_mut(26..28)?;
        mtu_target.copy_from_slice(&self.mtu_size.to_be_bytes());
        Some(needed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenConnectionRequest2 {
    pub server_address: SocketAddr,
    pub mtu_size: u16,
    pub client_guid: u64,
}

impl OpenConnectionRequest2 {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::OPEN_CONNECTION_REQUEST_2 {
            return None;
        }
        let magic_slice = buf.get(1..17)?;
        if !is_valid_magic(magic_slice) {
            return None;
        }
        let rest = buf.get(17..)?;
        let (server_address, remaining) = AddressCodec::read_from(rest)?;
        let mtu_slice = remaining.get(..2)?;
        let guid_slice = remaining.get(2..10)?;
        let mtu_size = u16::from_be_bytes(mtu_slice.try_into().ok()?);
        let client_guid = u64::from_be_bytes(guid_slice.try_into().ok()?);
        Some(Self {
            server_address,
            mtu_size,
            client_guid,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let total_len = buf.len();
        if total_len < 17 {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::OPEN_CONNECTION_REQUEST_2;
        }
        let magic_target = buf.get_mut(1..17)?;
        magic_target.copy_from_slice(&RAKNET_MAGIC);
        let rest = buf.get_mut(17..)?;
        let written = AddressCodec::write_to(self.server_address, rest)?;
        let remaining = rest.get_mut(written..)?;

        let mtu_target = remaining.get_mut(..2)?;
        mtu_target.copy_from_slice(&self.mtu_size.to_be_bytes());
        let guid_target = remaining.get_mut(2..10)?;
        guid_target.copy_from_slice(&self.client_guid.to_be_bytes());
        let rest_len = remaining.get(10..).map_or(0, |s| s.len());
        Some(total_len - rest_len)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenConnectionReply2 {
    pub server_guid: u64,
    pub client_address: SocketAddr,
    pub mtu_size: u16,
    pub security: bool,
}

impl OpenConnectionReply2 {
    #[must_use]
    pub fn read_from(buf: &[u8]) -> Option<Self> {
        if *buf.first()? != id::OPEN_CONNECTION_REPLY_2 {
            return None;
        }
        let magic_slice = buf.get(1..17)?;
        if !is_valid_magic(magic_slice) {
            return None;
        }
        let guid_slice = buf.get(17..25)?;
        let server_guid = u64::from_be_bytes(guid_slice.try_into().ok()?);
        let rest = buf.get(25..)?;
        let (client_address, remaining) = AddressCodec::read_from(rest)?;
        let mtu_slice = remaining.get(..2)?;
        let sec_byte = *remaining.get(2)?;
        let mtu_size = u16::from_be_bytes(mtu_slice.try_into().ok()?);
        let security = sec_byte != 0;
        Some(Self {
            server_guid,
            client_address,
            mtu_size,
            security,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) -> Option<usize> {
        let total_len = buf.len();
        if total_len < 25 {
            return None;
        }
        if let Some(slot) = buf.first_mut() {
            *slot = id::OPEN_CONNECTION_REPLY_2;
        }
        let magic_target = buf.get_mut(1..17)?;
        magic_target.copy_from_slice(&RAKNET_MAGIC);
        let guid_target = buf.get_mut(17..25)?;
        guid_target.copy_from_slice(&self.server_guid.to_be_bytes());
        let rest = buf.get_mut(25..)?;
        let written = AddressCodec::write_to(self.client_address, rest)?;
        let remaining = rest.get_mut(written..)?;

        let mtu_target = remaining.get_mut(..2)?;
        mtu_target.copy_from_slice(&self.mtu_size.to_be_bytes());
        if let Some(slot) = remaining.get_mut(2) {
            *slot = if self.security { 1 } else { 0 };
        }
        let rest_len = remaining.get(3..).map_or(0, |s| s.len());
        Some(total_len - rest_len)
    }
}
