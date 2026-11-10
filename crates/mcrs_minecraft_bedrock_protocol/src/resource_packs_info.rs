use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

use crate::packet_header::{read_packet_header, write_packet_header};
use crate::packet_ids;

/// Resource pack entry in `ResourcePacksInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePackInfoEntry {
    pub uuid: String,
    pub version: String,
    pub size: u64,
    pub content_key: String,
    pub sub_pack_name: String,
    pub content_identity: String,
    pub has_scripts: bool,
}

impl BedrockEncode for ResourcePackInfoEntry {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.uuid.encode(w)?;
        self.version.encode(w)?;
        self.size.encode(w)?;
        self.content_key.encode(w)?;
        self.sub_pack_name.encode(w)?;
        self.content_identity.encode(w)?;
        self.has_scripts.encode(w)?;
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for ResourcePackInfoEntry {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let uuid = String::decode(r)?;
        let version = String::decode(r)?;
        let size = u64::decode(r)?;
        let content_key = String::decode(r)?;
        let sub_pack_name = String::decode(r)?;
        let content_identity = String::decode(r)?;
        let has_scripts = bool::decode(r)?;
        Ok(Self {
            uuid,
            version,
            size,
            content_key,
            sub_pack_name,
            content_identity,
            has_scripts,
        })
    }
}

/// Server packet advertising available resource and behavior packs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePacksInfo {
    pub must_accept: bool,
    pub has_scripts: bool,
    pub force_server_packs: bool,
    pub behavior_packs: Vec<ResourcePackInfoEntry>,
    pub resource_packs: Vec<ResourcePackInfoEntry>,
}

impl BedrockEncode for ResourcePacksInfo {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::RESOURCE_PACKS_INFO, w)?;
        self.must_accept.encode(w)?;
        self.has_scripts.encode(w)?;
        self.force_server_packs.encode(w)?;
        self.behavior_packs.encode(w)?;
        self.resource_packs.encode(w)?;
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for ResourcePacksInfo {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::RESOURCE_PACKS_INFO)?;
        let must_accept = bool::decode(r)?;
        let has_scripts = bool::decode(r)?;
        let force_server_packs = bool::decode(r)?;
        let behavior_packs = Vec::<ResourcePackInfoEntry>::decode(r)?;
        let resource_packs = Vec::<ResourcePackInfoEntry>::decode(r)?;
        Ok(Self {
            must_accept,
            has_scripts,
            force_server_packs,
            behavior_packs,
            resource_packs,
        })
    }
}
