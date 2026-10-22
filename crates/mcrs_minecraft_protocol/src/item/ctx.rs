use std::cell::Cell;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::{Context, bail, ensure};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_item::component::{InlineRef, WireRef};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_registries::{
    IdSet, Key, Namer, Ref, Registries, RegistryType, Resolve, Resolved, Resolver,
};
use uuid::Uuid;

use crate::registry::{decode_holder_id, decode_registry_id, encode_holder_id, encode_registry_id};
use crate::text::Text;
use crate::{Bounded, Decode, Encode, VarInt, VarLong};

/// What the entries a wire value names are numbered against. Both ends of a connection number a
/// registry alike: a client holds the registries its server sent, in the order sent.
#[derive(Clone, Copy, Default)]
pub struct WireCtx<'a> {
    registries: Option<&'a Registries>,
    blocks: Option<&'a BlockDefinitions>,
    walking: bool,
}

impl<'a> WireCtx<'a> {
    /// No registries: only a value that names no entry crosses the wire.
    pub const NONE: WireCtx<'static> = WireCtx {
        registries: None,
        blocks: None,
        walking: false,
    };

    /// Reads a value only for its length: every reference reads as a placeholder.
    pub(crate) const WALKING: WireCtx<'static> = WireCtx {
        registries: None,
        blocks: None,
        walking: true,
    };

    pub fn new(registries: &'a Registries) -> Self {
        WireCtx {
            registries: Some(registries),
            ..WireCtx::NONE
        }
    }

    /// Block states cross the wire by state id, which only the block definitions number.
    pub fn with_blocks(mut self, blocks: &'a BlockDefinitions) -> Self {
        self.blocks = Some(blocks);
        self
    }

    pub(crate) fn walking(&self) -> bool {
        self.walking
    }

    pub(crate) fn registries(&self) -> anyhow::Result<&'a Registries> {
        self.registries
            .context("a registry reference needs the local registries")
    }

    pub(crate) fn blocks(&self) -> anyhow::Result<&'a BlockDefinitions> {
        self.blocks
            .context("a block state needs the block definitions")
    }
}

pub trait EncodeCtx {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()>;
}

pub trait DecodeCtx<'a>: Sized {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self>;
}

pub const MAX_NESTING: u32 = 64;

pub(crate) fn registries<'a>(ctx: &WireCtx<'a>) -> anyhow::Result<&'a Registries> {
    ctx.registries()
}

/// `value` in the named form, written as one network NBT tag.
pub(crate) fn encode_named_nbt<T: Resolve<Named: serde::Serialize>>(
    value: &T,
    ctx: &WireCtx<'_>,
    w: impl Write,
) -> anyhow::Result<()> {
    encode_nbt_wire(&value.name(registries(ctx)?)?, w)
}

/// A network NBT tag in the named form, resolved against the local registries.
pub(crate) fn decode_named_nbt<T: Resolve<Named: serde::de::DeserializeOwned> + Default>(
    ctx: &WireCtx<'_>,
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
                _: &$crate::item::ctx::WireCtx<'_>,
                w: impl std::io::Write,
            ) -> anyhow::Result<()> {
                $crate::Encode::encode(self, w)
            }
        }
        impl<'a> $crate::item::ctx::DecodeCtx<'a> for $ty {
            fn decode_ctx(
                _: &$crate::item::ctx::WireCtx<'_>,
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
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
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
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match bool::decode(r)? {
            true => Some(T::decode_ctx(ctx, r)?),
            false => None,
        })
    }
}

fn encode_items<T: EncodeCtx>(
    items: &[T],
    ctx: &WireCtx<'_>,
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
    ctx: &WireCtx<'_>,
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
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        encode_items(self, ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>> DecodeCtx<'a> for Vec<T> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_items(usize::MAX, ctx, r)
    }
}

impl<T: EncodeCtx, const MAX: usize> EncodeCtx for Bounded<Vec<T>, MAX> {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        ensure!(
            self.0.len() <= MAX,
            "list of {} entries exceeds the maximum of {MAX}",
            self.0.len()
        );
        encode_items(&self.0, ctx, w)
    }
}

impl<'a, T: DecodeCtx<'a>, const MAX: usize> DecodeCtx<'a> for Bounded<Vec<T>, MAX> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_items(MAX, ctx, r).map(Bounded)
    }
}

impl<T: EncodeCtx, const N: usize> EncodeCtx for [T; N] {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        for item in self {
            item.encode_ctx(ctx, &mut w)?;
        }
        Ok(())
    }
}

impl<'a, T: DecodeCtx<'a>, const N: usize> DecodeCtx<'a> for [T; N] {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
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
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(self.len() as i32).encode(&mut w)?;
        for (key, value) in self {
            key.encode_ctx(ctx, &mut w)?;
            value.encode_ctx(ctx, &mut w)?;
        }
        Ok(())
    }
}

impl<'a, K: DecodeCtx<'a> + PartialEq, V: DecodeCtx<'a>> DecodeCtx<'a> for Vec<(K, V)> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
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

impl<R: RegistryType> EncodeCtx for ResourceKey<R> {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        let id: Id<R> = Resolver::id(registries(ctx)?, &Key::new(self.location().clone()))
            .map_err(anyhow::Error::msg)?;
        encode_registry_id(id.number(), w)
    }
}

impl<'a, R: RegistryType> DecodeCtx<'a> for ResourceKey<R> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let id = local_id::<R>(decode_registry_id(r)?, ctx)?;
        if ctx.walking() {
            return Ok(ResourceKey::from_location(ResourceLocation::read(
                "mcrs:unresolved",
            )?));
        }
        let name = Namer::name(registries(ctx)?, id).context("a checked id has a name")?;
        Ok(ResourceKey::from_location(name.clone()))
    }
}

fn wire_number<R: RegistryType>(id: Id<R>) -> u16 {
    id.number()
}

fn local_id<R: RegistryType>(wire: u16, ctx: &WireCtx<'_>) -> anyhow::Result<Id<R>> {
    if ctx.walking() {
        return Id::checked(0, 1).context("a placeholder id");
    }
    let registries = registries(ctx)?;
    let registry = registries
        .key_of::<R>()
        .with_context(|| format!("no registry is bound to {}", std::any::type_name::<R>()))?;
    Id::checked(wire, usize::from(u16::MAX) + 1)
        .filter(|&id| Namer::name(registries, id).is_some())
        .with_context(|| format!("registry {registry} has no id {wire}"))
}

impl<R: RegistryType> EncodeCtx for Id<R> {
    fn encode_ctx(&self, _: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(wire_number(*self), w)
    }
}

impl<'a, R: RegistryType> DecodeCtx<'a> for Id<R> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        local_id(decode_registry_id(r)?, ctx)
    }
}

fn encode_ref<R: RegistryType, V: EncodeCtx>(
    reference: Result<Id<R>, &V>,
    ctx: &WireCtx<'_>,
    mut w: impl Write,
) -> anyhow::Result<()> {
    match reference {
        Ok(id) => encode_holder_id(Some(wire_number(id)), w),
        Err(value) => {
            encode_holder_id(None, &mut w)?;
            value.encode_ctx(ctx, w)
        }
    }
}

fn decode_ref<'a, R: RegistryType, V: DecodeCtx<'a>>(
    ctx: &WireCtx<'_>,
    r: &mut &'a [u8],
) -> anyhow::Result<Result<Id<R>, V>> {
    match decode_holder_id(r)? {
        Some(wire) => local_id(wire, ctx).map(Ok),
        None => V::decode_ctx(ctx, r).map(Err),
    }
}

impl<R: RegistryType, V: EncodeCtx> EncodeCtx for Ref<R, V> {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        match self {
            Ref::Id(id) => encode_ref::<R, V>(Ok(*id), ctx, w),
            Ref::Inline(value) => encode_ref::<R, V>(Err(value), ctx, w),
        }
    }
}

impl<'a, R: RegistryType, V: DecodeCtx<'a>> DecodeCtx<'a> for Ref<R, V> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match decode_ref::<R, V>(ctx, r)? {
            Ok(id) => Ref::Id(id),
            Err(value) => Ref::Inline(Arc::new(value)),
        })
    }
}

impl<R: RegistryType, V: EncodeCtx> EncodeCtx for InlineRef<Resolved, R, V> {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        match self {
            InlineRef::Id(id) => encode_ref::<R, V>(Ok(*id), ctx, w),
            InlineRef::Inline(value) => encode_ref::<R, V>(Err(value), ctx, w),
        }
    }
}

impl<'a, R: RegistryType, V: DecodeCtx<'a>> DecodeCtx<'a> for InlineRef<Resolved, R, V> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match decode_ref::<R, V>(ctx, r)? {
            Ok(id) => InlineRef::Id(id),
            Err(value) => InlineRef::Inline(Arc::new(value)),
        })
    }
}

impl<R: RegistryType, V: EncodeCtx> EncodeCtx for WireRef<Resolved, R, V> {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        self.0.encode_ctx(ctx, w)
    }
}

impl<'a, R: RegistryType, V: DecodeCtx<'a>> DecodeCtx<'a> for WireRef<Resolved, R, V> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        InlineRef::decode_ctx(ctx, r).map(WireRef)
    }
}

impl<R: RegistryType> EncodeCtx for IdSet<R> {
    fn encode_ctx(&self, ctx: &WireCtx<'_>, mut w: impl Write) -> anyhow::Result<()> {
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
                    encode_registry_id(wire_number(id), &mut w)?;
                }
                Ok(())
            }
        }
    }
}

impl<'a, R: RegistryType> DecodeCtx<'a> for IdSet<R> {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
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
    fn encode_ctx(&self, ctx: &WireCtx<'_>, w: impl Write) -> anyhow::Result<()> {
        encode_named_nbt(self, ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for mcrs_minecraft_item::Text {
    fn decode_ctx(ctx: &WireCtx<'_>, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_named_nbt(ctx, r)
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
    pub fn resolve(&self, ctx: &WireCtx<'_>) -> anyhow::Result<T> {
        let mut r = &self.0[..];
        let value = T::decode_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after a raw value", r.len());
        Ok(value)
    }

    pub fn from_value(value: &T, ctx: &WireCtx<'_>) -> anyhow::Result<Self> {
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
        T::decode_ctx(&WireCtx::WALKING, r)?;
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
