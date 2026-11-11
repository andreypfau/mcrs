use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32, VarU32};

use crate::item_stack::{NetworkItemStack, decode_network_item, encode_network_item};

/// One change of a slot in an [`ItemUseTransaction`](crate::ItemUseTransaction).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryAction<'a> {
    /// The source, with the fields that its type carries.
    pub source: InventorySource,
    /// The slot that changes.
    pub slot: VarU32,
    /// The item before the change; `None` is the empty item.
    pub old_item: Option<NetworkItemStack<'a>>,
    /// The item after the change; `None` is the empty item.
    pub new_item: Option<NetworkItemStack<'a>>,
}

/// The source type of an [`InventoryAction`]; another type is a decode error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InventorySource {
    /// Type 0: a container.
    Container {
        /// The container id.
        container_id: VarI32,
    },
    /// Type 1: global.
    Global,
    /// Type 2: a world interaction.
    WorldInteraction {
        /// The flags of the interaction.
        flags: VarU32,
    },
    /// Type 3: the creative inventory.
    Creative,
    /// Type 99999: an untracked interaction.
    UntrackedInteraction {
        /// The container id.
        container_id: VarI32,
    },
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
