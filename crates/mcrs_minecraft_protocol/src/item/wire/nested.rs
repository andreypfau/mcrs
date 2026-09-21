use std::io::Write;

use mcrs_minecraft_registry::RegistryLookup;

use crate::Encode;
use crate::item::component::nested::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::stack::Template;
use crate::item::wire::{newtype_ctx_wire, record_ctx_wire};

newtype_ctx_wire!(UseRemainder, SulfurCubeContent, BundleContents);
record_ctx_wire! {
    ChargedProjectiles { items },
    Container { slots },
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
