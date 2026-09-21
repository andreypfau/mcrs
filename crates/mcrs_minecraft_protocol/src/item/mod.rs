pub mod component;
pub mod ctx;
#[doc(hidden)]
pub mod harness;
pub mod hash_ops;
pub mod kind;
pub mod patch;
pub mod stack;
pub mod trade;
mod wire;

pub use component::*;
pub use ctx::{DecodeCtx, EncodeCtx, Raw};
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
