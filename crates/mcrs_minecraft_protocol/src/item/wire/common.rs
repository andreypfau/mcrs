use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registry::{Holder, HolderWireOnly, Registered, RegistryLookup, RegistryName};

use crate::item::component::common::*;
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::wire::{newtype_wire, ordinal_enum_wire, record_ctx_wire};
use crate::{Bounded, Decode, Encode, VarInt};

newtype_wire!(RgbInt, ArgbInt, NbtPredicate);
ordinal_enum_wire!(EquipmentSlotGroup, ItemUseAnimation);

impl<T: Registered + EncodeCtx> EncodeCtx for HolderWireOnly<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl<'a, T: Registered + DecodeCtx<'a>> DecodeCtx<'a> for HolderWireOnly<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Holder::decode_ctx(ctx, r).map(HolderWireOnly)
    }
}

impl<T: EncodeCtx> EncodeCtx for Filterable<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.raw.encode_ctx(ctx, &mut w)?;
        self.filtered.encode_ctx(ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>> DecodeCtx<'a> for Filterable<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Filterable {
            raw: T::decode_ctx(ctx, r)?,
            filtered: Option::decode_ctx(ctx, r)?,
        })
    }
}

macro_rules! resolvable_wire {
    ($($name:ident),* $(,)?) => {$(
        impl EncodeCtx for $name {
            fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
                match self {
                    Self::Constant(value) => {
                        true.encode(&mut w)?;
                        value.encode(w)
                    }
                    Self::Reference(key) => {
                        false.encode(&mut w)?;
                        key.location().encode(w)
                    }
                }
            }
        }

        impl DecodeCtx<'_> for $name {
            fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
                Ok(match bool::decode(r)? {
                    true => Self::Constant(Decode::decode(r)?),
                    false => {
                        Self::Reference(ResourceKey::from_location(ResourceLocation::decode(r)?))
                    }
                })
            }
        }
    )*};
}

pub(crate) use resolvable_wire;

resolvable_wire!(ResolvableInt, ResolvableFloat);

record_ctx_wire!(MobEffectInstance { id, details });

impl EncodeCtx for MobEffectDetails {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.amplifier() as i32).encode(&mut w)?;
        VarInt(self.duration).encode(&mut w)?;
        self.ambient.encode(&mut w)?;
        self.show_particles.encode(&mut w)?;
        self.show_icon.encode(&mut w)?;
        match &self.hidden_effect {
            Some(hidden) => {
                true.encode(&mut w)?;
                hidden.encode_ctx(ctx, w)
            }
            None => false.encode(w),
        }
    }
}

impl DecodeCtx<'_> for MobEffectDetails {
    #[allow(clippy::only_used_in_recursion)]
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let amplifier = VarInt::decode(r)?.0.clamp(0, 255);
        let duration = VarInt::decode(r)?.0;
        let ambient = bool::decode(r)?;
        let show_particles = bool::decode(r)?;
        let show_icon = bool::decode(r)?;
        let hidden_effect = match bool::decode(r)? {
            true => Some(Box::new(MobEffectDetails::decode_ctx(ctx, r)?)),
            false => None,
        };
        Ok(MobEffectDetails {
            amplifier: amplifier as i8,
            duration,
            ambient,
            show_particles,
            show_icon,
            hidden_effect,
        })
    }
}

impl<R: RegistryName> EncodeCtx for TypedEntityData<R> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode_ctx(ctx, &mut w)?;
        self.tag.encode(w)
    }
}

impl<R: RegistryName> DecodeCtx<'_> for TypedEntityData<R> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let id = ResourceKey::decode_ctx(ctx, r)?;
        ensure!(
            r.first() == Some(&mcrs_minecraft_nbt::COMPOUND_ID),
            "expected a compound tag"
        );
        Ok(TypedEntityData {
            id,
            tag: NbtCompound::decode(r)?,
        })
    }
}

impl<const MAX_CHARS: usize> Encode for BoundedString<MAX_CHARS> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        Bounded::<&str, MAX_CHARS>(&self.0).encode(w)
    }
}

impl<const MAX_CHARS: usize> Decode<'_> for BoundedString<MAX_CHARS> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(BoundedString(
            Bounded::<&str, MAX_CHARS>::decode(r)?.0.into(),
        ))
    }
}

impl<const MAX_CHARS: usize> EncodeCtx for BoundedString<MAX_CHARS> {
    fn encode_ctx(&self, _: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.encode(w)
    }
}

impl<const MAX_CHARS: usize> DecodeCtx<'_> for BoundedString<MAX_CHARS> {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Self::decode(r)
    }
}

impl<T: EncodeCtx> EncodeCtx for CompactList<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>> DecodeCtx<'a> for CompactList<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Vec::decode_ctx(ctx, r).map(CompactList)
    }
}

impl EncodeCtx for ValueMatcher {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            ValueMatcher::Exact(value) => {
                true.encode(&mut w)?;
                value.encode(w)
            }
            ValueMatcher::Ranged { min, max } => {
                false.encode(&mut w)?;
                min.encode(&mut w)?;
                max.encode(w)
            }
        }
    }
}

impl DecodeCtx<'_> for ValueMatcher {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => ValueMatcher::Exact(String::decode(r)?),
            false => ValueMatcher::Ranged {
                min: Option::decode(r)?,
                max: Option::decode(r)?,
            },
        })
    }
}
