use alloc::vec::Vec;
use core::num::NonZeroI32;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarI32, VarU32};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack<'a> {
    pub network_id: NonZeroI32,
    pub count: u16,
    pub metadata: VarU32,
    pub block_runtime_id: VarI32,
    pub user_data: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkItemStack<'a> {
    pub item: ItemStack<'a>,
    pub stack_network_id: Option<VarI32>,
}

#[derive(Clone, Copy)]
enum StackNetworkId {
    Absent,
    Present,
}

pub(crate) fn encode_item(item: Option<&ItemStack<'_>>, w: &mut Vec<u8>) -> Result<(), Error> {
    encode_parts(item, None, StackNetworkId::Absent, w)
}

pub(crate) fn decode_item<'a>(r: &mut &'a [u8]) -> Result<Option<ItemStack<'a>>, Error> {
    Ok(decode_parts(StackNetworkId::Absent, r)?.map(|network_item| network_item.item))
}

pub(crate) fn encode_network_item(
    item: Option<&NetworkItemStack<'_>>,
    w: &mut Vec<u8>,
) -> Result<(), Error> {
    let stack_network_id = item.and_then(|network_item| network_item.stack_network_id);
    let item = item.map(|network_item| &network_item.item);
    encode_parts(item, stack_network_id, StackNetworkId::Present, w)
}

pub(crate) fn decode_network_item<'a>(
    r: &mut &'a [u8],
) -> Result<Option<NetworkItemStack<'a>>, Error> {
    decode_parts(StackNetworkId::Present, r)
}

fn encode_parts(
    item: Option<&ItemStack<'_>>,
    stack_network_id: Option<VarI32>,
    form: StackNetworkId,
    w: &mut Vec<u8>,
) -> Result<(), Error> {
    let Some(item) = item else {
        return VarI32(0).encode(w);
    };
    VarI32(item.network_id.get()).encode(w)?;
    item.count.encode(w)?;
    item.metadata.encode(w)?;
    if let StackNetworkId::Present = form {
        stack_network_id.encode(w)?;
    }
    item.block_runtime_id.encode(w)?;
    item.user_data.encode(w)
}

fn decode_parts<'a>(
    form: StackNetworkId,
    r: &mut &'a [u8],
) -> Result<Option<NetworkItemStack<'a>>, Error> {
    let VarI32(network_id) = VarI32::decode(r)?;
    let Some(network_id) = NonZeroI32::new(network_id) else {
        return Ok(None);
    };
    let count = u16::decode(r)?;
    let metadata = VarU32::decode(r)?;
    let stack_network_id = match form {
        StackNetworkId::Absent => None,
        StackNetworkId::Present => Option::<VarI32>::decode(r)?,
    };
    let item = ItemStack {
        network_id,
        count,
        metadata,
        block_runtime_id: VarI32::decode(r)?,
        user_data: BedrockDecode::decode(r)?,
    };
    Ok(Some(NetworkItemStack {
        item,
        stack_network_id,
    }))
}
