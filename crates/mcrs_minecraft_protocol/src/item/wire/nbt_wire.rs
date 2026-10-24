use std::io::Write;

use crate::item::component::nbt_wire::*;
use crate::item::lookup::{decode_nbt_wire, encode_nbt_wire};
use crate::item::wire::newtype_wire;

newtype_wire!(CustomData, BucketEntityData);

/// A kind whose wire form is its persistent form as one network NBT tag.
macro_rules! nbt_wire {
    ($($ty:ident),* $(,)?) => {$(
        impl crate::Encode for $ty {
            fn encode(&self, w: impl Write) -> anyhow::Result<()> {
                encode_nbt_wire(self, w)
            }
        }

        impl crate::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                decode_nbt_wire(r)
            }
        }
    )*};
}

nbt_wire!(MapDecorations, DebugStickState, Recipes, ContainerLoot);
