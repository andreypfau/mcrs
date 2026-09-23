use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::codec;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::book::*;
use crate::item::component::common::Filterable;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::record_ctx_wire;
use crate::{Decode, Encode, VarInt};

record_ctx_wire!(WritableBookContent { pages });

impl EncodeCtx for WrittenBookContent {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.title.encode_ctx(ctx, &mut w)?;
        self.author.encode(&mut w)?;
        VarInt(self.generation.0).encode(&mut w)?;
        self.pages.encode_ctx(ctx, &mut w)?;
        self.resolved.encode(w)
    }
}

impl DecodeCtx<'_> for WrittenBookContent {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let title = Filterable::decode_ctx(ctx, r)?;
        let author = String::decode(r)?;
        let generation = VarInt::decode(r)?.0;
        ensure!(
            (0..=3).contains(&generation),
            "Generation was {generation}, but must be between 0 and 3"
        );
        Ok(WrittenBookContent {
            title,
            author,
            generation: codec::Bounded(generation),
            pages: Vec::decode_ctx(ctx, r)?,
            resolved: bool::decode(r)?,
        })
    }
}
