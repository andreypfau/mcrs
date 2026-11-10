use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error};

use crate::packet_header::{read_packet_header, write_packet_header};
use crate::packet_ids;

/// A pack in `ResourcePackStack`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePackStackEntry {
    pub uuid: String,
    pub version: String,
    pub sub_pack_name: String,
}

impl BedrockEncode for ResourcePackStackEntry {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.uuid.encode(w)?;
        self.version.encode(w)?;
        self.sub_pack_name.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for ResourcePackStackEntry {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(Self {
            uuid: String::decode(r)?,
            version: String::decode(r)?,
            sub_pack_name: String::decode(r)?,
        })
    }
}

/// An experiment toggle in `ResourcePackStack`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Experiment {
    pub name: String,
    pub enabled: bool,
}

impl BedrockEncode for Experiment {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.name.encode(w)?;
        self.enabled.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for Experiment {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(Self {
            name: String::decode(r)?,
            enabled: bool::decode(r)?,
        })
    }
}

/// The server sets the order of the packs that the client applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourcePackStack {
    pub must_accept: bool,
    pub behavior_packs: Vec<ResourcePackStackEntry>,
    pub resource_packs: Vec<ResourcePackStackEntry>,
    pub game_version: String,
    pub experiments: Vec<Experiment>,
    pub were_experiments_previously_enabled: bool,
}

impl BedrockEncode for ResourcePackStack {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::RESOURCE_PACK_STACK, w)?;
        self.must_accept.encode(w)?;
        self.behavior_packs.encode(w)?;
        self.resource_packs.encode(w)?;
        self.game_version.encode(w)?;
        // The experiment count is a little-endian u32, not the varint of other lists.
        let count = u32::try_from(self.experiments.len()).map_err(|_| Error::LengthOverflow)?;
        count.encode(w)?;
        for experiment in &self.experiments {
            experiment.encode(w)?;
        }
        self.were_experiments_previously_enabled.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for ResourcePackStack {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::RESOURCE_PACK_STACK)?;
        let must_accept = bool::decode(r)?;
        let behavior_packs = Vec::decode(r)?;
        let resource_packs = Vec::decode(r)?;
        let game_version = String::decode(r)?;
        let count = usize::try_from(u32::decode(r)?).map_err(|_| Error::LengthOverflow)?;
        let mut experiments = Vec::with_capacity(count.min(64));
        for _ in 0..count {
            experiments.push(Experiment::decode(r)?);
        }
        Ok(Self {
            must_accept,
            behavior_packs,
            resource_packs,
            game_version,
            experiments,
            were_experiments_previously_enabled: bool::decode(r)?,
        })
    }
}
