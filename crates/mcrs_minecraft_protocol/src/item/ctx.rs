use std::io::Write;
use std::marker::PhantomData;
use std::sync::{Arc, LazyLock};

use anyhow::{Context, bail, ensure};
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registry::{ItemId, RegistryLookup};
use uuid::Uuid;

use crate::entity::DyeColor;
use crate::item::component::{Holder, Registered, RegistryName};
use crate::item::kind::ItemComponentKind;
use crate::text::Text;
use crate::{Bounded, Decode, Encode, VarInt, VarLong};

pub trait EncodeCtx {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()>;
}

pub trait DecodeCtx<'a>: Sized {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self>;
}

macro_rules! ctx_free {
    ($($ty:ty),* $(,)?) => {$(
        impl $crate::item::ctx::EncodeCtx for $ty {
            fn encode_ctx(
                &self,
                _: &dyn mcrs_minecraft_registry::RegistryLookup,
                w: impl std::io::Write,
            ) -> anyhow::Result<()> {
                $crate::Encode::encode(self, w)
            }
        }
        impl<'a> $crate::item::ctx::DecodeCtx<'a> for $ty {
            fn decode_ctx(
                _: &dyn mcrs_minecraft_registry::RegistryLookup,
                r: &mut &'a [u8],
            ) -> anyhow::Result<Self> {
                $crate::Decode::decode(r)
            }
        }
    )*};
}
pub(crate) use ctx_free;

ctx_free!(
    bool,
    u8,
    i8,
    i16,
    i32,
    i64,
    f32,
    f64,
    String,
    VarInt,
    VarLong,
    Uuid,
    Text,
    NbtCompound,
    ResourceLocation<Arc<str>>,
    ItemId,
    ItemComponentKind,
    DyeColor,
);

impl<T: EncodeCtx> EncodeCtx for Option<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            Some(value) => {
                true.encode(&mut w)?;
                value.encode_ctx(ctx, w)
            }
            None => false.encode(w),
        }
    }
}

impl<'a, T: DecodeCtx<'a>> DecodeCtx<'a> for Option<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => Some(T::decode_ctx(ctx, r)?),
            false => None,
        })
    }
}

fn encode_items<T: EncodeCtx>(
    items: &[T],
    ctx: &dyn RegistryLookup,
    mut w: impl Write,
) -> anyhow::Result<()> {
    VarInt(items.len() as i32).encode(&mut w)?;
    for item in items {
        item.encode_ctx(ctx, &mut w)?;
    }
    Ok(())
}

fn decode_items<'a, T: DecodeCtx<'a>>(
    max: usize,
    ctx: &dyn RegistryLookup,
    r: &mut &'a [u8],
) -> anyhow::Result<Vec<T>> {
    let len = VarInt::decode(r)?.0;
    ensure!(len >= 0, "attempt to decode a list with negative length");
    let len = len as usize;
    ensure!(
        len <= max,
        "list of {len} entries exceeds the maximum of {max}"
    );
    let mut items = Vec::with_capacity(len.min(r.len()));
    for _ in 0..len {
        items.push(T::decode_ctx(ctx, r)?);
    }
    Ok(items)
}

impl<T: EncodeCtx> EncodeCtx for Vec<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        encode_items(self, ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>> DecodeCtx<'a> for Vec<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_items(usize::MAX, ctx, r)
    }
}

impl<T: EncodeCtx, const MAX: usize> EncodeCtx for Bounded<Vec<T>, MAX> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        ensure!(
            self.0.len() <= MAX,
            "list of {} entries exceeds the maximum of {MAX}",
            self.0.len()
        );
        encode_items(&self.0, ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>, const MAX: usize> DecodeCtx<'a> for Bounded<Vec<T>, MAX> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_items(MAX, ctx, r).map(Bounded)
    }
}

impl<T: EncodeCtx, const N: usize> EncodeCtx for [T; N] {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        for item in self {
            item.encode_ctx(ctx, &mut w)?;
        }
        Ok(())
    }
}

impl<'a, T: DecodeCtx<'a>, const N: usize> DecodeCtx<'a> for [T; N] {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let mut items = Vec::with_capacity(N);
        for _ in 0..N {
            items.push(T::decode_ctx(ctx, r)?);
        }
        Ok(items
            .try_into()
            .unwrap_or_else(|_| unreachable!("exactly {N} items were decoded")))
    }
}

impl<K: EncodeCtx, V: EncodeCtx> EncodeCtx for Vec<(K, V)> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.len() as i32).encode(&mut w)?;
        for (key, value) in self {
            key.encode_ctx(ctx, &mut w)?;
            value.encode_ctx(ctx, &mut w)?;
        }
        Ok(())
    }
}

impl<'a, K: DecodeCtx<'a> + PartialEq, V: DecodeCtx<'a>> DecodeCtx<'a> for Vec<(K, V)> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode a map with negative length");
        let mut entries: Vec<(K, V)> = Vec::with_capacity((len as usize).min(r.len()));
        for _ in 0..len {
            let key = K::decode_ctx(ctx, r)?;
            let value = V::decode_ctx(ctx, r)?;
            ensure!(
                entries.iter().all(|(k, _)| *k != key),
                "duplicate key in a wire map"
            );
            entries.push((key, value));
        }
        Ok(entries)
    }
}

impl<R: RegistryName> EncodeCtx for ResourceKey<R> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        let id = ctx
            .id(R::NAME, self.location())
            .with_context(|| format!("{self} is not in registry {}", R::NAME))?;
        VarInt(id as i32).encode(w)
    }
}

impl<'a, R: RegistryName> DecodeCtx<'a> for ResourceKey<R> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let name = u32::try_from(id)
            .ok()
            .and_then(|id| ctx.name(R::NAME, id))
            .with_context(|| format!("registry {} has no id {id}", R::NAME))?;
        Ok(ResourceKey::from_location(name.clone()))
    }
}

impl<T: Registered> EncodeCtx for Holder<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            Holder::Reference(key) => {
                let id = ctx
                    .id(T::Registry::NAME, key.location())
                    .with_context(|| format!("{key} is not in registry {}", T::Registry::NAME))?;
                VarInt(id as i32 + 1).encode(w)
            }
            Holder::Direct(value) => {
                VarInt(0).encode(&mut w)?;
                value.encode_ctx(ctx, w)
            }
        }
    }
}

impl<'a, T: Registered> DecodeCtx<'a> for Holder<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let raw = VarInt::decode(r)?.0;
        if raw == 0 {
            return T::decode_ctx(ctx, r).map(Holder::Direct);
        }
        let id = raw.wrapping_sub(1);
        let name = u32::try_from(id)
            .ok()
            .and_then(|id| ctx.name(T::Registry::NAME, id))
            .with_context(|| format!("registry {} has no id {id}", T::Registry::NAME))?;
        Ok(Holder::Reference(ResourceKey::from_location(name.clone())))
    }
}

impl<R: RegistryName, const L: bool> EncodeCtx for HolderSet<ResourceKey<R>, L> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            HolderSet::Tag(tag) => {
                VarInt(0).encode(&mut w)?;
                tag.encode(w)
            }
            _ => {
                let entries = self.entries();
                VarInt(entries.len() as i32 + 1).encode(&mut w)?;
                for entry in entries {
                    entry.encode_ctx(ctx, &mut w)?;
                }
                Ok(())
            }
        }
    }
}

impl<'a, R: RegistryName, const L: bool> DecodeCtx<'a> for HolderSet<ResourceKey<R>, L> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let raw = VarInt::decode(r)?.0;
        ensure!(raw >= 0, "holder set with negative length");
        if raw == 0 {
            return Ok(HolderSet::Tag(ResourceLocation::decode(r)?));
        }
        let len = raw as usize - 1;
        if len == 1 && !L {
            return Ok(HolderSet::One(ResourceKey::decode_ctx(ctx, r)?));
        }
        let mut entries = Vec::with_capacity(len.min(r.len()));
        for _ in 0..len {
            entries.push(ResourceKey::decode_ctx(ctx, r)?);
        }
        Ok(HolderSet::List(entries))
    }
}

static UNRESOLVED: LazyLock<ResourceLocation> =
    LazyLock::new(|| ResourceLocation::new("mcrs", "unresolved"));

/// Resolves every id and every name, so a stack can be walked for its length
/// without the registries. Sound only while no wire layout in the dispatch
/// table depends on which entry an id names.
pub(crate) struct Opaque;

impl RegistryLookup for Opaque {
    fn id(&self, _: &str, _: &ResourceLocation) -> Option<u32> {
        Some(0)
    }

    fn name(&self, _: &str, _: u32) -> Option<&ResourceLocation> {
        Some(&UNRESOLVED)
    }
}

/// The exact bytes of one registry-dependent value, kept so a packet can
/// carry it without the registries; the value walks the layout to find its
/// length and is resolved on demand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raw<T>(pub bytes::Bytes, PhantomData<T>);

impl<T: EncodeCtx + for<'a> DecodeCtx<'a>> Raw<T> {
    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<T> {
        let mut r = &self.0[..];
        let value = T::decode_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after a raw value", r.len());
        Ok(value)
    }

    pub fn from_value(value: &T, ctx: &dyn RegistryLookup) -> anyhow::Result<Self> {
        let mut bytes = Vec::new();
        value.encode_ctx(ctx, &mut bytes)?;
        Ok(Raw(bytes.into(), PhantomData))
    }
}

impl<T> Encode for Raw<T> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl<T: for<'a> DecodeCtx<'a>> Decode<'_> for Raw<T> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        T::decode_ctx(&Opaque, r)?;
        Ok(Raw(
            bytes::Bytes::copy_from_slice(&start[..start.len() - r.len()]),
            PhantomData,
        ))
    }
}

pub(crate) fn encode_nbt_wire<T: serde::Serialize>(value: &T, w: impl Write) -> anyhow::Result<()> {
    mcrs_minecraft_nbt::to_bytes_unnamed(value, w)?;
    Ok(())
}

pub(crate) fn decode_nbt_wire<T: serde::de::DeserializeOwned>(r: &mut &[u8]) -> anyhow::Result<T> {
    match r.first() {
        None => bail!("empty input for a network NBT tag"),
        Some(&mcrs_minecraft_nbt::END_ID) => bail!("a network NBT tag must not be TAG_End"),
        Some(_) => {}
    }
    let mut cursor = std::io::Cursor::new(*r);
    let value = mcrs_minecraft_nbt::from_bytes_unnamed(&mut cursor)?;
    *r = &r[cursor.position() as usize..];
    Ok(value)
}
