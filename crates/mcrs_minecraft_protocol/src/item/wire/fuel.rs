use std::io::Write;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::fuel::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::{Decode, Encode};

impl EncodeCtx for ResolvableNumber {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            ResolvableNumber::Constant(value) => {
                true.encode(&mut w)?;
                value.encode(w)
            }
            ResolvableNumber::Reference(key) => {
                false.encode(&mut w)?;
                key.location().encode(w)
            }
        }
    }
}

impl DecodeCtx<'_> for ResolvableNumber {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => ResolvableNumber::Constant(f32::decode(r)?),
            false => ResolvableNumber::reference(ResourceLocation::decode(r)?),
        })
    }
}

macro_rules! fuel_wire {
    ($($ty:ident { $($field:ident),+ }),* $(,)?) => {$(
        impl EncodeCtx for $ty {
            fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
                $(self.$field.encode_ctx(ctx, &mut w)?;)+
                Ok(())
            }
        }

        impl DecodeCtx<'_> for $ty {
            fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok($ty {
                    $($field: ResolvableNumber::decode_ctx(ctx, r)?,)+
                })
            }
        }
    )*};
}

fuel_wire! {
    Compostable { layers },
    CookingFuel { burn_time, speed_multiplier },
    BrewingFuel { uses, speed_multiplier },
}
