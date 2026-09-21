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
