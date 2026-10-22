use std::io::Write;

use crate::item::ctx::WireCtx;

use crate::item::component::common::TypedEntityData;
use crate::item::component::entity_data::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::newtype_ctx_wire;
use crate::{Decode, Encode, VarInt};

newtype_ctx_wire!(EntityData, BlockEntityData, Bees);

impl EncodeCtx for BeeOccupant {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        self.entity_data.encode_ctx(ctx, &mut w)?;
        VarInt(self.ticks_in_hive).encode(&mut w)?;
        VarInt(self.min_ticks_in_hive).encode(w)
    }
}

impl DecodeCtx<'_> for BeeOccupant {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BeeOccupant {
            entity_data: TypedEntityData::decode_ctx(ctx, r)?,
            ticks_in_hive: VarInt::decode(r)?.0,
            min_ticks_in_hive: VarInt::decode(r)?.0,
        })
    }
}
