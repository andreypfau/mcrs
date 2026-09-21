pub mod component;
pub mod ctx;
#[doc(hidden)]
pub mod harness;
pub mod hash_ops;
pub mod kind;
pub mod patch;
pub mod stack;
pub mod trade;
pub(crate) mod wire;

pub use component::*;
pub use ctx::{DecodeCtx, EncodeCtx, Raw};
pub use wire::{decode_component_value, decode_delimited_patch, encode_delimited_patch};
pub use kind::{ItemComponentKind, ItemComponentValue, ItemDataComponent};
pub use patch::{ComponentMap, ComponentPatch};
pub use stack::{
    HashedPatchMap, HashedSlot, ItemStackValue, ItemStackWithSlot, RawDelimitedStack, RawStack,
    Slot, Template,
};
pub use trade::{ItemCost, MerchantOffer, RawMerchantOffer};

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
