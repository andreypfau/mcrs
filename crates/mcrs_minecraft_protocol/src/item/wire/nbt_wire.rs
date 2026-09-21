use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::nbt_wire::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx, decode_nbt_wire, encode_nbt_wire};
use crate::item::wire::newtype_wire;

newtype_wire!(CustomData, BucketEntityData);

/// A kind whose wire form is its persistent form as one network NBT tag.
macro_rules! nbt_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl EncodeCtx for $ty {
            fn encode_ctx(&self, _: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
                encode_nbt_wire(self, w)
            }
        }

        impl DecodeCtx<'_> for $ty {
            fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
                decode_nbt_wire(r)
            }
        }
    )*};
}

nbt_wire!(MapDecorations, DebugStickState, Recipes, ContainerLoot);
