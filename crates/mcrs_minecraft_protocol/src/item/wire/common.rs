use mcrs_minecraft_registries::Wire;
use std::io::Write;

use anyhow::{Context, ensure};
use mcrs_minecraft_block::keys::BlockEntityType;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::item::component::common::*;
use crate::item::lookup::nested;
use crate::item::wire::{newtype_wire, ordinal_enum_wire, record_wire};
use crate::{Bounded, Decode, Encode, VarInt};

newtype_wire!(RgbInt, ArgbInt, NbtPredicate);
ordinal_enum_wire!(EquipmentSlotGroup, ItemUseAnimation);

impl<T: Encode> Encode for Filterable<T> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.raw.encode(&mut w)?;
        self.filtered.encode(w)
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for Filterable<T> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Filterable {
            raw: T::decode(r)?,
            filtered: Option::decode(r)?,
        })
    }
}

macro_rules! resolvable_wire {
    ($($name:ident),* $(,)?) => {$(
        impl Encode for $name {
            fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
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

        impl Decode<'_> for $name {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
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

resolvable_wire!(ResolvableInt, ResolvableFloat);

record_wire!(MobEffectInstance<Wire> { id, details });

impl Encode for MobEffectDetails {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.amplifier() as i32).encode(&mut w)?;
        VarInt(self.duration).encode(&mut w)?;
        self.ambient.encode(&mut w)?;
        self.show_particles.encode(&mut w)?;
        self.show_icon.encode(&mut w)?;
        match &self.hidden_effect {
            Some(hidden) => {
                true.encode(&mut w)?;
                hidden.encode(w)
            }
            None => false.encode(w),
        }
    }
}

impl Decode<'_> for MobEffectDetails {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let amplifier = VarInt::decode(r)?.0.clamp(0, 255);
        let duration = VarInt::decode(r)?.0;
        let ambient = bool::decode(r)?;
        let show_particles = bool::decode(r)?;
        let show_icon = bool::decode(r)?;
        let hidden_effect = match bool::decode(r)? {
            true => Some(Box::new(nested(|| MobEffectDetails::decode(r))?)),
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

/// The type a typed entity data names is an entry of a static registry, numbered without the
/// registries.
macro_rules! typed_entity_data_wire {
    ($($registry:ty),* $(,)?) => {$(
        impl Encode for TypedEntityData<$registry> {
            fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
                <$registry>::find(self.id.location().as_str())
                    .with_context(|| format!("no {} is named {}", stringify!($registry), self.id))?
                    .encode(&mut w)?;
                self.tag.encode(w)
            }
        }

        impl Decode<'_> for TypedEntityData<$registry> {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let entry = <$registry>::decode(r)?;
                ensure!(
                    r.first() == Some(&mcrs_minecraft_nbt::COMPOUND_ID),
                    "expected a compound tag"
                );
                Ok(TypedEntityData {
                    id: ResourceKey::from_location(entry.location().to_arc()),
                    tag: NbtCompound::decode(r)?,
                })
            }
        }
    )*};
}

typed_entity_data_wire!(EntityType, BlockEntityType);

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

impl<T: Encode> Encode for CompactList<T> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.0.encode(w)
    }
}

impl<'a, T: Decode<'a>> Decode<'a> for CompactList<T> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Vec::decode(r).map(CompactList)
    }
}

impl Encode for ValueMatcher {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
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

impl Decode<'_> for ValueMatcher {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => ValueMatcher::Exact(String::decode(r)?),
            false => ValueMatcher::Ranged {
                min: Option::decode(r)?,
                max: Option::decode(r)?,
            },
        })
    }
}
