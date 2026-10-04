use std::cell::Cell;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::{Arc, LazyLock};

use anyhow::{Context, bail, ensure};
use mcrs_minecraft_core::{HolderSet, RegistryKey, ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registry::{ItemId, RegistryLookup};
use uuid::Uuid;

use crate::item::component::Holder;
use crate::text::Text;
use crate::{Bounded, Decode, Encode, VarInt, VarLong};

pub trait EncodeCtx {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()>;
}

pub trait DecodeCtx<'a>: Sized {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self>;
}

pub const MAX_NESTING: u32 = 64;

thread_local! {
    static NESTING: Cell<u32> = const { Cell::new(0) };
}

/// Bounds the recursion of a self-containing wire value, since the layout
/// alone lets a few kilobytes of wrappers overflow the stack. A Java client
/// only loses the connection to that; a Rust one would abort.
pub(crate) fn nested<T>(decode: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    struct Unwind(u32);
    impl Drop for Unwind {
        fn drop(&mut self) {
            NESTING.set(self.0);
        }
    }
    let depth = NESTING.get();
    ensure!(
        depth < MAX_NESTING,
        "value nested deeper than {MAX_NESTING} levels"
    );
    NESTING.set(depth + 1);
    let _unwind = Unwind(depth);
    decode()
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

impl<R: RegistryKey> EncodeCtx for ResourceKey<R> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        let id = ctx
            .id(R::KEY.path(), self.location())
            .with_context(|| format!("{self} is not in registry {}", R::KEY.path()))?;
        VarInt(id as i32).encode(w)
    }
}

impl<'a, R: RegistryKey> DecodeCtx<'a> for ResourceKey<R> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let id = VarInt::decode(r)?.0;
        let name = u32::try_from(id)
            .ok()
            .and_then(|id| ctx.name(R::KEY.path(), id))
            .with_context(|| format!("registry {} has no id {id}", R::KEY.path()))?;
        Ok(ResourceKey::from_location(name.clone()))
    }
}

impl<T: RegistryKey + EncodeCtx> EncodeCtx for Holder<T> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            Holder::Reference(key) => {
                let id = ctx
                    .id(T::KEY.path(), key.location())
                    .with_context(|| format!("{key} is not in registry {}", T::KEY.path()))?;
                VarInt(id as i32 + 1).encode(w)
            }
            Holder::Direct(value) => {
                VarInt(0).encode(&mut w)?;
                value.encode_ctx(ctx, w)
            }
        }
    }
}

impl<'a, T: RegistryKey + DecodeCtx<'a>> DecodeCtx<'a> for Holder<T> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let raw = VarInt::decode(r)?.0;
        if raw == 0 {
            return T::decode_ctx(ctx, r).map(Holder::Direct);
        }
        let id = raw.wrapping_sub(1);
        let name = u32::try_from(id)
            .ok()
            .and_then(|id| ctx.name(T::KEY.path(), id))
            .with_context(|| format!("registry {} has no id {id}", T::KEY.path()))?;
        Ok(Holder::Reference(ResourceKey::from_location(name.clone())))
    }
}

impl<R: RegistryKey, const L: bool> EncodeCtx for HolderSet<ResourceKey<R>, L> {
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

impl<'a, R: RegistryKey, const L: bool> DecodeCtx<'a> for HolderSet<ResourceKey<R>, L> {
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

    fn block_state_id(&self, _: &ResourceLocation, _: &[(&str, &str)]) -> Option<u32> {
        Some(0)
    }

    fn block_state(&self, _: u32) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        Some((UNRESOLVED.clone(), Vec::new()))
    }
}

/// The exact bytes of one registry-dependent value, kept so a packet can
/// carry it without the registries; the value walks the layout to find its
/// length and is resolved on demand.
pub struct Raw<T>(pub bytes::Bytes, pub(crate) PhantomData<T>);

impl<T> Clone for Raw<T> {
    fn clone(&self) -> Self {
        Raw(self.0.clone(), PhantomData)
    }
}

impl<T> PartialEq for Raw<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Raw<T> {}

impl<T> std::fmt::Debug for Raw<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Raw").field(&self.0).finish()
    }
}

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

type NbtWireDeserializer<'x, 'y> =
    mcrs_minecraft_nbt::deserializer::Deserializer<&'x mut std::io::Cursor<&'y [u8]>>;

pub(crate) fn read_nbt_wire<T>(
    r: &mut &[u8],
    read: impl FnOnce(&mut NbtWireDeserializer<'_, '_>) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    match r.first() {
        None => bail!("empty input for a network NBT tag"),
        Some(&mcrs_minecraft_nbt::END_ID) => bail!("a network NBT tag must not be TAG_End"),
        Some(_) => {}
    }
    let mut cursor = std::io::Cursor::new(*r);
    let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
    let value = read(&mut d)?;
    *r = &r[cursor.position() as usize..];
    Ok(value)
}

pub(crate) fn decode_nbt_wire<T: serde::de::DeserializeOwned>(r: &mut &[u8]) -> anyhow::Result<T> {
    read_nbt_wire(r, |d| Ok(T::deserialize(&mut *d)?))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use mcrs_minecraft_item::Item;
    use mcrs_minecraft_registry::{LookupIndex, NoRegistries};

    use super::*;
    use crate::item::component::SoundEvent;

    struct Recording {
        index: LookupIndex,
        asked: Mutex<Vec<String>>,
    }

    impl Recording {
        fn new(registry: &str, names: &[&str]) -> Self {
            let mut index = LookupIndex::default();
            for (id, name) in names.iter().enumerate() {
                index.insert(registry, id as u32, Some(ResourceLocation::minecraft(name)));
            }
            Recording {
                index,
                asked: Mutex::new(Vec::new()),
            }
        }

        fn asked(&self) -> Vec<String> {
            self.asked.lock().unwrap().clone()
        }
    }

    impl RegistryLookup for Recording {
        fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
            self.asked.lock().unwrap().push(registry.to_owned());
            self.index.id(registry, name)
        }

        fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
            self.asked.lock().unwrap().push(registry.to_owned());
            self.index.name(registry, id)
        }
    }

    #[test]
    fn a_reference_is_looked_up_by_the_bare_registry_path() {
        let lookup = Recording::new("item", &["air", "stone"]);
        let key = ResourceKey::<Item>::from_location(ResourceLocation::minecraft("stone"));
        let mut bytes = Vec::new();
        key.encode_ctx(&lookup, &mut bytes).unwrap();
        assert_eq!(lookup.asked(), ["item"]);

        let decoded = ResourceKey::<Item>::decode_ctx(&lookup, &mut &bytes[..]).unwrap();
        assert_eq!(decoded, key);
        assert_eq!(lookup.asked(), ["item", "item"]);
    }

    #[test]
    fn a_holder_reference_is_looked_up_by_the_bare_registry_path() {
        let lookup = Recording::new("sound_event", &["a", "b"]);
        let holder = Holder::<SoundEvent>::reference(ResourceLocation::minecraft("b"));
        let mut bytes = Vec::new();
        holder.encode_ctx(&lookup, &mut bytes).unwrap();
        let decoded = Holder::<SoundEvent>::decode_ctx(&lookup, &mut &bytes[..]).unwrap();
        assert_eq!(decoded, holder);
        assert_eq!(lookup.asked(), ["sound_event", "sound_event"]);
    }

    #[test]
    fn a_missing_id_names_the_registry_by_its_bare_path() {
        let error = ResourceKey::<Item>::decode_ctx(&NoRegistries, &mut &[5u8][..]).unwrap_err();
        assert!(
            error.to_string().contains("registry item has no id 5"),
            "{error}"
        );

        let error = Holder::<SoundEvent>::decode_ctx(&NoRegistries, &mut &[6u8][..]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("registry sound_event has no id 5"),
            "{error}"
        );
    }
}
