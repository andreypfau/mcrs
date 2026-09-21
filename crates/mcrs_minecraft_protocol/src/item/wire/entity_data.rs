use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::common::TypedEntityData;
use crate::item::component::entity_data::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::newtype_ctx_wire;
use crate::{Decode, Encode, VarInt};

newtype_ctx_wire!(EntityData, BlockEntityData);

impl EncodeCtx for BeeOccupant {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.entity_data.encode_ctx(ctx, &mut w)?;
        VarInt(self.ticks_in_hive).encode(&mut w)?;
        VarInt(self.min_ticks_in_hive).encode(w)
    }
}

impl DecodeCtx<'_> for BeeOccupant {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BeeOccupant {
            entity_data: TypedEntityData::decode_ctx(ctx, r)?,
            ticks_in_hive: VarInt::decode(r)?.0,
            min_ticks_in_hive: VarInt::decode(r)?.0,
        })
    }
}

impl EncodeCtx for Bees {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl DecodeCtx<'_> for Bees {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(Bees)
    }
}
