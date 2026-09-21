use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::nested::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::stack::Template;
use crate::item::wire::newtype_ctx_wire;
use crate::{Bounded, Encode};

newtype_ctx_wire!(UseRemainder, SulfurCubeContent, BundleContents);

impl EncodeCtx for ChargedProjectiles {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.items.encode_ctx(ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for ChargedProjectiles {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self {
            items: Bounded::decode_ctx(ctx, r)?,
        })
    }
}

impl EncodeCtx for PotDecorations {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        for side in self.sides() {
            match side {
                Some(template) => {
                    true.encode(&mut w)?;
                    template.encode_ctx(ctx, &mut w)?;
                }
                None => false.encode(&mut w)?,
            }
        }
        Ok(())
    }
}

impl<'a> DecodeCtx<'a> for PotDecorations {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let mut side = || Option::<Template>::decode_ctx(ctx, r).map(|t| t.map(Box::new));
        Ok(PotDecorations {
            back: side()?,
            left: side()?,
            right: side()?,
            front: side()?,
        })
    }
}

impl EncodeCtx for Container {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.slots.encode_ctx(ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for Container {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self {
            slots: Bounded::decode_ctx(ctx, r)?,
        })
    }
}
