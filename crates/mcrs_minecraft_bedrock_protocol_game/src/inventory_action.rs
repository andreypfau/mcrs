use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32, VarU32};

use crate::item_stack::{NetworkItemStack, decode_network_item, encode_network_item};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryAction<'a> {
    pub source: InventorySource,
    pub slot: VarU32,
    pub old_item: Option<NetworkItemStack<'a>>,
    pub new_item: Option<NetworkItemStack<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InventorySource {
    Container { container_id: VarI32 },
    Global,
    WorldInteraction { flags: VarU32 },
    Creative,
    UntrackedInteraction { container_id: VarI32 },
}

impl BedrockEncode for InventoryAction<'_> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match self.source {
            InventorySource::Container { container_id } => {
                VarU32(0).encode(w)?;
                container_id.encode(w)?;
            }
            InventorySource::Global => VarU32(1).encode(w)?,
            InventorySource::WorldInteraction { flags } => {
                VarU32(2).encode(w)?;
                flags.encode(w)?;
            }
            InventorySource::Creative => VarU32(3).encode(w)?,
            InventorySource::UntrackedInteraction { container_id } => {
                VarU32(99_999).encode(w)?;
                container_id.encode(w)?;
            }
        }
        self.slot.encode(w)?;
        encode_network_item(self.old_item.as_ref(), w)?;
        encode_network_item(self.new_item.as_ref(), w)
    }
}

impl<'a> BedrockDecode<'a> for InventoryAction<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let source = match VarU32::decode(r)? {
            VarU32(0) => InventorySource::Container {
                container_id: VarI32::decode(r)?,
            },
            VarU32(1) => InventorySource::Global,
            VarU32(2) => InventorySource::WorldInteraction {
                flags: VarU32::decode(r)?,
            },
            VarU32(3) => InventorySource::Creative,
            VarU32(99_999) => InventorySource::UntrackedInteraction {
                container_id: VarI32::decode(r)?,
            },
            VarU32(other) => return Err(Error::UnknownEnumValue(i64::from(other))),
        };
        Ok(Self {
            source,
            slot: VarU32::decode(r)?,
            old_item: decode_network_item(r)?,
            new_item: decode_network_item(r)?,
        })
    }
}
