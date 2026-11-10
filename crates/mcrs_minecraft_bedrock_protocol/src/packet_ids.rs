//! Packet ids of the handshake, login and resource pack negotiation.

pub const LOGIN: u32 = 0x01;
pub const PLAY_STATUS: u32 = 0x02;
pub const DISCONNECT: u32 = 0x05;
pub const RESOURCE_PACKS_INFO: u32 = 0x06;
pub const RESOURCE_PACK_STACK: u32 = 0x07;
pub const RESOURCE_PACK_CLIENT_RESPONSE: u32 = 0x08;
pub const NETWORK_SETTINGS: u32 = 0x8F;
pub const REQUEST_NETWORK_SETTINGS: u32 = 0xC1;
