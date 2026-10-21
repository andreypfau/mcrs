use std::cell::Cell;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::{Arc, LazyLock};

use anyhow::{Context, bail, ensure};
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::{Registered, ResourceKey, ResourceLocation, rl};
use mcrs_minecraft_item::component::{InlineRef, WireRef};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registries::{
    IdSet, Key, Namer, Ref, RegistryType, Resolve, Resolved, Resolver,
};
use mcrs_minecraft_registry::{RegistryLookup, RegistrySet};
use uuid::Uuid;

use crate::registry::{decode_holder_id, decode_registry_id, encode_holder_id, encode_registry_id};
use crate::text::Text;
use crate::{Bounded, Decode, Encode, VarInt, VarLong};

pub trait EncodeCtx {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()>;
}

pub trait DecodeCtx<'a>: Sized {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self>;
}

pub const MAX_NESTING: u32 = 64;

pub(crate) fn registries(ctx: &dyn RegistryLookup) -> anyhow::Result<&RegistrySet> {
    ctx.registries()
        .context("a registry reference needs the local registries")
}

/// `value` in the named form, written as one network NBT tag.
pub(crate) fn encode_named_nbt<T: Resolve<Named: serde::Serialize>>(
    value: &T,
    ctx: &dyn RegistryLookup,
    w: impl Write,
) -> anyhow::Result<()> {
    encode_nbt_wire(&value.name(registries(ctx)?)?, w)
}

/// A network NBT tag in the named form, resolved against the local registries.
pub(crate) fn decode_named_nbt<T: Resolve<Named: serde::de::DeserializeOwned> + Default>(
    ctx: &dyn RegistryLookup,
    r: &mut &[u8],
) -> anyhow::Result<T> {
    let named = decode_nbt_wire::<T::Named>(r)?;
    if ctx.walking() {
        return Ok(T::default());
    }
    Ok(T::resolve(named, registries(ctx)?)?)
}

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

impl<R: Registered> EncodeCtx for ResourceKey<R> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        let id = ctx
            .id(R::REGISTRY.path(), self.location())
            .with_context(|| format!("{self} is not in registry {}", R::REGISTRY.path()))?;
        encode_registry_id(id, w)
    }
}

impl<'a, R: Registered> DecodeCtx<'a> for ResourceKey<R> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let id = decode_registry_id(r)
            .with_context(|| format!("registry {} has no id that wide", R::REGISTRY.path()))?;
        let name = ctx
            .name(R::REGISTRY.path(), id)
            .with_context(|| format!("registry {} has no id {id}", R::REGISTRY.path()))?;
        Ok(ResourceKey::from_location(name.clone()))
    }
}

fn wire_number<R: RegistryType>(id: Id<R>, ctx: &dyn RegistryLookup) -> anyhow::Result<u16> {
    let set = registries(ctx)?;
    let registry = set
        .registry_name::<R>()
        .with_context(|| format!("no registry is bound to {}", std::any::type_name::<R>()))?;
    let name =
        Namer::name(set, id).with_context(|| format!("{id:?} is not in registry {registry}"))?;
    ctx.id(registry.path(), name)
        .with_context(|| format!("{name} is not in registry {registry}"))
}

fn local_id<R: RegistryType>(wire: u16, ctx: &dyn RegistryLookup) -> anyhow::Result<Id<R>> {
    if ctx.walking() {
        return Id::checked(0, 1).context("a placeholder id");
    }
    let set = registries(ctx)?;
    let registry = set
        .registry_name::<R>()
        .with_context(|| format!("no registry is bound to {}", std::any::type_name::<R>()))?;
    let name = ctx
        .name(registry.path(), wire)
        .with_context(|| format!("registry {} has no id {wire}", registry.path()))?;
    Resolver::id(set, &Key::new(name.clone())).map_err(anyhow::Error::msg)
}

impl<R: RegistryType> EncodeCtx for Id<R> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(wire_number(*self, ctx)?, w)
    }
}

impl<'a, R: RegistryType> DecodeCtx<'a> for Id<R> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        local_id(decode_registry_id(r)?, ctx)
    }
}

fn encode_ref<R: RegistryType, V: EncodeCtx>(
    reference: Result<Id<R>, &V>,
    ctx: &dyn RegistryLookup,
    mut w: impl Write,
) -> anyhow::Result<()> {
    match reference {
        Ok(id) => encode_holder_id(Some(wire_number(id, ctx)?), w),
        Err(value) => {
            encode_holder_id(None, &mut w)?;
            value.encode_ctx(ctx, w)
        }
    }
}

fn decode_ref<'a, R: RegistryType, V: DecodeCtx<'a>>(
    ctx: &dyn RegistryLookup,
    r: &mut &'a [u8],
) -> anyhow::Result<Result<Id<R>, V>> {
    match decode_holder_id(r)? {
        Some(wire) => local_id(wire, ctx).map(Ok),
        None => V::decode_ctx(ctx, r).map(Err),
    }
}

impl<R: RegistryType, V: EncodeCtx> EncodeCtx for Ref<R, V> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        match self {
            Ref::Id(id) => encode_ref::<R, V>(Ok(*id), ctx, w),
            Ref::Inline(value) => encode_ref::<R, V>(Err(value), ctx, w),
        }
    }
}

impl<'a, R: RegistryType, V: DecodeCtx<'a>> DecodeCtx<'a> for Ref<R, V> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match decode_ref::<R, V>(ctx, r)? {
            Ok(id) => Ref::Id(id),
            Err(value) => Ref::Inline(Arc::new(value)),
        })
    }
}

impl<R: RegistryType, V: EncodeCtx> EncodeCtx for InlineRef<Resolved, R, V> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        match self {
            InlineRef::Id(id) => encode_ref::<R, V>(Ok(*id), ctx, w),
            InlineRef::Inline(value) => encode_ref::<R, V>(Err(value), ctx, w),
        }
    }
}

impl<'a, R: RegistryType, V: DecodeCtx<'a>> DecodeCtx<'a> for InlineRef<Resolved, R, V> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match decode_ref::<R, V>(ctx, r)? {
            Ok(id) => InlineRef::Id(id),
            Err(value) => InlineRef::Inline(Arc::new(value)),
        })
    }
}

impl<R: RegistryType, V: EncodeCtx> EncodeCtx for WireRef<Resolved, R, V> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl<'a, R: RegistryType, V: DecodeCtx<'a>> DecodeCtx<'a> for WireRef<Resolved, R, V> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        InlineRef::decode_ctx(ctx, r).map(WireRef)
    }
}

impl<R: RegistryType> EncodeCtx for IdSet<R> {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            IdSet::Tag(tag) => {
                VarInt(0).encode(&mut w)?;
                registries(ctx)?
                    .tag_name(*tag)
                    .with_context(|| format!("no tag is numbered {}", tag.number()))?
                    .encode(w)
            }
            IdSet::List(ids) => {
                VarInt(ids.len() as i32 + 1).encode(&mut w)?;
                for &id in ids.iter() {
                    encode_registry_id(wire_number(id, ctx)?, &mut w)?;
                }
                Ok(())
            }
        }
    }
}

impl<'a, R: RegistryType> DecodeCtx<'a> for IdSet<R> {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let raw = VarInt::decode(r)?.0;
        ensure!(raw >= 0, "holder set with negative length");
        if raw == 0 {
            let name = ResourceLocation::<Arc<str>>::decode(r)?;
            if ctx.walking() {
                return Ok(IdSet::List(Box::new([])));
            }
            return registries(ctx)?
                .tag(&Key::new(name))
                .map(IdSet::Tag)
                .map_err(anyhow::Error::msg);
        }
        let len = raw as usize - 1;
        let mut ids = Vec::with_capacity(len.min(r.len()));
        for _ in 0..len {
            ids.push(Id::<R>::decode_ctx(ctx, r)?);
        }
        Ok(IdSet::List(ids.into()))
    }
}

/// A text a stack component holds: one network NBT tag of its named form.
impl EncodeCtx for mcrs_minecraft_item::Text {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        encode_named_nbt(self, ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for mcrs_minecraft_item::Text {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_named_nbt(ctx, r)
    }
}

static UNRESOLVED: LazyLock<ResourceLocation> = LazyLock::new(|| rl!("mcrs:unresolved").to_arc());

/// Resolves every id and every name, so a stack can be walked for its length
/// without the registries. Sound only while no wire layout in the dispatch
/// table depends on which entry an id names.
pub(crate) struct Opaque;

impl RegistryLookup for Opaque {
    fn id(&self, _: &str, _: &ResourceLocation) -> Option<u16> {
        Some(0)
    }

    fn walking(&self) -> bool {
        true
    }

    fn name(&self, _: &str, _: u16) -> Option<&ResourceLocation> {
        Some(&UNRESOLVED)
    }

    fn block_state_id(&self, _: &ResourceLocation, _: &[(&str, &str)]) -> Option<u16> {
        Some(0)
    }

    fn block_state(&self, _: u16) -> Option<(ResourceLocation, Vec<(String, String)>)> {
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
    use mcrs_minecraft_item::keys::Item;
    use mcrs_minecraft_registry::{LookupIndex, NoRegistries, Registry, RegistrySet};

    use super::*;
    use mcrs_minecraft_sound::SoundEvent;

    struct Indexed {
        index: LookupIndex,
        local: RegistrySet,
    }

    impl RegistryLookup for Indexed {
        fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u16> {
            self.index.id(registry, name)
        }

        fn name(&self, registry: &str, id: u16) -> Option<&ResourceLocation> {
            self.index.name(registry, id)
        }

        fn registries(&self) -> Option<&RegistrySet> {
            Some(&self.local)
        }
    }

    fn lookup(registry: &str, names: &[&str]) -> Indexed {
        let mut index = LookupIndex::default();
        for (id, name) in (0u16..).zip(names) {
            index.insert(
                registry,
                id,
                Some(ResourceLocation::minecraft(name).unwrap()),
            );
        }
        Indexed {
            index,
            local: RegistrySet::new(),
        }
    }

    fn with_local<R: Registered>(mut lookup: Indexed, names: &[&str]) -> Indexed {
        let registry = Registry::<R>::new(
            R::REGISTRY,
            names
                .iter()
                .map(|name| ResourceLocation::minecraft(name).unwrap()),
        )
        .unwrap();
        lookup.local = lookup.local.with(registry).unwrap();
        lookup
    }

    #[test]
    fn a_reference_is_looked_up_by_the_bare_registry_path() {
        let lookup = lookup("item", &["air", "stone"]);
        let key = ResourceKey::<Item>::from_location(rl!("minecraft:stone").to_arc());
        let mut bytes = Vec::new();
        key.encode_ctx(&lookup, &mut bytes).unwrap();

        let decoded = ResourceKey::<Item>::decode_ctx(&lookup, &mut &bytes[..]).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn a_holder_reference_crosses_the_wire_by_name_between_two_numberings() {
        let lookup = with_local::<mcrs_minecraft_sound::SoundEvent>(
            lookup("sound_event", &["a", "b", "c"]),
            &["c", "a", "b"],
        );
        let local = lookup
            .local
            .registry::<mcrs_minecraft_sound::SoundEvent>()
            .unwrap();
        let holder = Ref::<SoundEvent, SoundEvent>::Id(local.by_name("minecraft:b").unwrap());
        assert_eq!(local.by_name("minecraft:b").unwrap().number(), 2);

        let mut bytes = Vec::new();
        holder.encode_ctx(&lookup, &mut bytes).unwrap();
        assert_eq!(decode_holder_id(&mut &bytes[..]).unwrap(), Some(1));
        let decoded = Ref::<SoundEvent, SoundEvent>::decode_ctx(&lookup, &mut &bytes[..]).unwrap();
        assert_eq!(decoded, holder);
    }

    #[test]
    fn a_wire_number_the_local_registry_cannot_name_does_not_decode() {
        let lookup = with_local::<mcrs_minecraft_sound::SoundEvent>(
            lookup("sound_event", &["a", "b", "c"]),
            &["a", "b"],
        );
        let mut bytes = Vec::new();
        encode_holder_id(Some(2), &mut bytes).unwrap();
        let error =
            Ref::<SoundEvent, SoundEvent>::decode_ctx(&lookup, &mut &bytes[..]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown entry minecraft:c of minecraft:sound_event"),
            "{error}"
        );
    }

    #[test]
    fn a_missing_id_names_the_registry_by_its_bare_path() {
        let error = ResourceKey::<Item>::decode_ctx(&NoRegistries, &mut &[5u8][..]).unwrap_err();
        assert!(
            error.to_string().contains("registry item has no id 5"),
            "{error}"
        );

        let sounds = with_local::<SoundEvent>(lookup("sound_event", &["a"]), &["a"]);
        let error =
            Ref::<SoundEvent, SoundEvent>::decode_ctx(&sounds, &mut &[6u8][..]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("registry sound_event has no id 5"),
            "{error}"
        );
    }
}
