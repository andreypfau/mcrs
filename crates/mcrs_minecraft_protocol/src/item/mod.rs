pub mod ctx;
pub mod trade;
pub(crate) mod wire;

pub use ctx::{DecodeCtx, EncodeCtx, Raw};
pub use mcrs_minecraft_item_component::*;
pub use mcrs_minecraft_item_component::{component, harness, hash_ops, kind, patch, stack};
pub use trade::{ItemCost, MerchantOffer, RawMerchantOffer};
pub use wire::{HashedStack, ProtoStack, RawDelimitedStack, RawStack};
pub use wire::{decode_component_value, decode_delimited_patch, encode_delimited_patch};

use crate::{Decode, Encode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ContainerInput {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum QuickCraftKind {
    Split = 0,
    Single = 1,
    Full = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum QuickCraftStage {
    Header = 0,
    Slot = 1,
    End = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickCraftButton {
    pub kind: QuickCraftKind,
    pub stage: QuickCraftStage,
}

impl TryFrom<u8> for QuickCraftButton {
    type Error = u8;

    fn try_from(button: u8) -> Result<Self, u8> {
        let kind = match button >> 2 & 3 {
            0 => QuickCraftKind::Split,
            1 => QuickCraftKind::Single,
            2 => QuickCraftKind::Full,
            _ => return Err(button),
        };
        let stage = match button & 3 {
            0 => QuickCraftStage::Header,
            1 => QuickCraftStage::Slot,
            2 => QuickCraftStage::End,
            _ => return Err(button),
        };
        Ok(QuickCraftButton { kind, stage })
    }
}

impl From<QuickCraftButton> for u8 {
    fn from(button: QuickCraftButton) -> u8 {
        button.stage as u8 & 3 | (button.kind as u8 & 3) << 2
    }
}
