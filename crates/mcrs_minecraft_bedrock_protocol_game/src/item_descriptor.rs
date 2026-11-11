use alloc::vec::Vec;
use core::num::NonZeroI16;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32};

/// An ingredient of an auto craft recipe: an item descriptor and a count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
pub struct Ingredient<'a> {
    /// The items that the ingredient accepts.
    pub descriptor: ItemDescriptor<'a>,
    /// The number of items.
    pub count: VarI32,
}

/// The items that an [`Ingredient`] accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ItemDescriptor<'a> {
    /// Type 0: no item.
    Empty,
    /// Type 1: an item network id; `None` writes the id 0 and no metadata.
    Default(Option<DefaultItem>),
    /// Type 2: a molang expression.
    MolangExpression {
        /// The expression.
        expression: &'a str,
        /// The molang version.
        version: u8,
    },
    /// Type 3: an item tag.
    ItemTag(&'a str),
    /// Type 4: a full item name with metadata.
    FullName {
        /// The full name of the item.
        name: &'a str,
        /// The metadata of the item.
        metadata: i16,
    },
    /// Type 5: a complex alias name.
    ComplexAlias(&'a str),
}

/// The item of [`ItemDescriptor::Default`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DefaultItem {
    /// The item network id.
    pub network_id: NonZeroI16,
    /// The metadata of the item.
    pub metadata: i16,
}

impl BedrockEncode for ItemDescriptor<'_> {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Empty => 0u8.encode(w),
            Self::Default(item) => {
                1u8.encode(w)?;
                match item {
                    Some(item) => {
                        item.network_id.get().encode(w)?;
                        item.metadata.encode(w)
                    }
                    None => 0i16.encode(w),
                }
            }
            Self::MolangExpression {
                expression,
                version,
            } => {
                2u8.encode(w)?;
                expression.encode(w)?;
                version.encode(w)
            }
            Self::ItemTag(tag) => {
                3u8.encode(w)?;
                tag.encode(w)
            }
            Self::FullName { name, metadata } => {
                4u8.encode(w)?;
                name.encode(w)?;
                metadata.encode(w)
            }
            Self::ComplexAlias(name) => {
                5u8.encode(w)?;
                name.encode(w)
            }
        }
    }
}

impl<'a> BedrockDecode<'a> for ItemDescriptor<'a> {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(match u8::decode(r)? {
            0 => Self::Empty,
            1 => match NonZeroI16::new(i16::decode(r)?) {
                Some(network_id) => Self::Default(Some(DefaultItem {
                    network_id,
                    metadata: i16::decode(r)?,
                })),
                None => Self::Default(None),
            },
            2 => Self::MolangExpression {
                expression: BedrockDecode::decode(r)?,
                version: u8::decode(r)?,
            },
            3 => Self::ItemTag(BedrockDecode::decode(r)?),
            4 => Self::FullName {
                name: BedrockDecode::decode(r)?,
                metadata: i16::decode(r)?,
            },
            5 => Self::ComplexAlias(BedrockDecode::decode(r)?),
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        })
    }
}
