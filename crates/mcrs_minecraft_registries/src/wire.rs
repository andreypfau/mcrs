use std::fmt;
use std::io::Write;
use std::marker::PhantomData;
use std::sync::Arc;

use derive_where::derive_where;
use mcrs_minecraft_core::Bounded;
use mcrs_minecraft_core::codec::CompactList;
use mcrs_minecraft_resource::{Id, Registered, ResourceLocationBuf, TagKeyBuf};
use mcrs_minecraft_wire::{Context, Decode, Encode, ErrorKind, Result, VarInt};

use crate::Namer;
use crate::refs::{Checked, Named, Ref, Refs, ResolveError, Resolved, Resolver};
use crate::static_key::StaticEntry;
use crate::tags::IdSet;

/// The form a value crosses the network in: an id is the sixteen-bit number the peer sent, not
/// yet checked against any registry, and a tag is its name. A value that the wire writes as named
/// NBT keeps its [`Named`] form inside.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Wire;

impl Refs for Wire {
    type Id<R: Registered> = WireId<R>;
    type Set<R: Registered> = WireSet<R>;
    type Nbt = Named;
    type Value<T: Checked> = T::Wire;

    fn id_of<R: StaticEntry>(entry: R) -> WireId<R> {
        entry.id().into()
    }
}

#[derive_where(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WireId<R> {
    number: u16,
    registry: PhantomData<fn() -> R>,
}

impl<R> WireId<R> {
    pub const fn new(number: u16) -> Self {
        WireId {
            number,
            registry: PhantomData,
        }
    }

    pub const fn number(self) -> u16 {
        self.number
    }
}

impl<R> From<Id<R>> for WireId<R> {
    fn from(id: Id<R>) -> Self {
        WireId::new(id.number())
    }
}

impl<R> fmt::Debug for WireId<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.number)
    }
}

#[derive_where(Clone, PartialEq, Eq, Debug)]
pub enum WireSet<R> {
    Tag(TagKeyBuf<R>),
    List(Box<[WireId<R>]>),
}

/// The block states, which the block definitions number and no registry holds.
pub trait BlockStates {
    fn block_state(
        &self,
        id: u16,
    ) -> Result<(ResourceLocationBuf, Vec<(String, String)>), ResolveError>;

    fn block_state_id(
        &self,
        block: &ResourceLocationBuf,
        properties: &[(&str, &str)],
    ) -> Result<u16, ResolveError>;
}

/// What a wire value is checked against: the registries both ends number alike, and the block
/// states.
pub trait WireNames: Resolver + Namer + BlockStates {}

impl<C: Resolver + Namer + BlockStates> WireNames for C {}

/// A value whose wire form is checked against the registries before it is trusted. Only this
/// step reads the registries; the wire form encodes and decodes without them.
pub trait FromWire: Sized {
    type Wire;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError>;

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError>;
}

impl<R: Registered> FromWire for Id<R> {
    type Wire = WireId<R>;

    fn from_wire<C: WireNames>(wire: WireId<R>, ctx: &C) -> Result<Self, ResolveError> {
        Id::checked(wire.number, usize::from(u16::MAX) + 1)
            .filter(|&id| ctx.name(id).is_some())
            .ok_or_else(|| {
                ResolveError::new(format!(
                    "registry {} has no id {}",
                    R::REGISTRY,
                    wire.number
                ))
            })
    }

    fn to_wire<C: WireNames>(&self, _: &C) -> Result<WireId<R>, ResolveError> {
        Ok((*self).into())
    }
}

impl<R: Registered> FromWire for IdSet<R> {
    type Wire = WireSet<R>;

    fn from_wire<C: WireNames>(wire: WireSet<R>, ctx: &C) -> Result<Self, ResolveError> {
        match wire {
            WireSet::Tag(tag) => ctx.tag(&tag).map(IdSet::Tag),
            WireSet::List(ids) => {
                Vec::from_wire(ids.into_vec(), ctx).map(|ids| IdSet::List(ids.into()))
            }
        }
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<WireSet<R>, ResolveError> {
        match self {
            IdSet::Tag(tag) => ctx
                .tag_name(*tag)
                .map(|name| WireSet::Tag(TagKeyBuf::from_location(name.clone())))
                .ok_or_else(|| ResolveError::new(format!("no tag is numbered {}", tag.number()))),
            IdSet::List(ids) => Ok(WireSet::List(
                ids.iter().copied().map(WireId::from).collect(),
            )),
        }
    }
}

impl<R: Registered, V: FromWire<Wire: Clone>> FromWire for Ref<Resolved, R, V> {
    type Wire = Ref<Wire, R, V::Wire>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        match wire {
            Ref::Id(id) => Id::from_wire(id, ctx).map(Ref::Id),
            Ref::Inline(value) => V::from_wire(Arc::unwrap_or_clone(value), ctx)
                .map(|value| Ref::Inline(Arc::new(value))),
        }
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        match self {
            Ref::Id(id) => Ok(Ref::Id((*id).into())),
            Ref::Inline(value) => value.to_wire(ctx).map(|value| Ref::Inline(Arc::new(value))),
        }
    }
}

impl<T: FromWire> FromWire for Option<T> {
    type Wire = Option<T::Wire>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        wire.map(|value| T::from_wire(value, ctx)).transpose()
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        self.as_ref().map(|value| value.to_wire(ctx)).transpose()
    }
}

impl<T: FromWire> FromWire for Vec<T> {
    type Wire = Vec<T::Wire>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        wire.into_iter()
            .enumerate()
            .map(|(at, value)| T::from_wire(value, ctx).map_err(|e| e.at_index(at)))
            .collect()
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        slice_to_wire(self, ctx)
    }
}

fn slice_to_wire<T: FromWire, C: WireNames>(
    values: &[T],
    ctx: &C,
) -> Result<Vec<T::Wire>, ResolveError> {
    values
        .iter()
        .enumerate()
        .map(|(at, value)| value.to_wire(ctx).map_err(|e| e.at_index(at)))
        .collect()
}

impl<T: FromWire, const N: usize> FromWire for [T; N] {
    type Wire = [T::Wire; N];

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        let values: Vec<T> = Vec::from_wire(wire.into(), ctx)?;
        values
            .try_into()
            .map_err(|_| ResolveError::new(format!("expected {N} values")))
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        slice_to_wire(self, ctx)?
            .try_into()
            .map_err(|_| ResolveError::new(format!("expected {N} values")))
    }
}

impl<T: FromWire, const MAX: usize> FromWire for Bounded<T, MAX> {
    type Wire = Bounded<T::Wire, MAX>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        T::from_wire(wire.0, ctx).map(Bounded)
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        self.0.to_wire(ctx).map(Bounded)
    }
}

impl<T: FromWire> FromWire for CompactList<T> {
    type Wire = CompactList<T::Wire>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        Vec::from_wire(wire.0, ctx).map(CompactList)
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        self.0.to_wire(ctx).map(CompactList)
    }
}

impl<T: FromWire> FromWire for Box<T> {
    type Wire = Box<T::Wire>;

    fn from_wire<C: WireNames>(wire: Self::Wire, ctx: &C) -> Result<Self, ResolveError> {
        T::from_wire(*wire, ctx).map(Box::new)
    }

    fn to_wire<C: WireNames>(&self, ctx: &C) -> Result<Self::Wire, ResolveError> {
        (**self).to_wire(ctx).map(Box::new)
    }
}

/// Implements [`FromWire`] for a value that the wire writes as named NBT: its wire form is its
/// [`Named`] form, so the check is [`Resolve`].
#[macro_export]
macro_rules! wire_as_named {
    (impl<$param:ident: $bound:path> $value:ty) => {
        $crate::wire_as_named!(@impl [$param: $bound] $value);
    };
    ($value:ty) => {
        $crate::wire_as_named!(@impl [] $value);
    };
    (@impl [$($generics:tt)*] $value:ty) => {
        impl<$($generics)*> $crate::FromWire for $value
        where
            $value: $crate::Resolve,
        {
            type Wire = <$value as $crate::Resolve>::Named;

            fn from_wire<C: $crate::WireNames>(
                wire: Self::Wire,
                ctx: &C,
            ) -> ::core::result::Result<Self, $crate::ResolveError> {
                <$value as $crate::Resolve>::resolve(wire, ctx)
            }

            fn to_wire<C: $crate::WireNames>(
                &self,
                ctx: &C,
            ) -> ::core::result::Result<Self::Wire, $crate::ResolveError> {
                <$value as $crate::Resolve>::name(self, ctx)
            }
        }
    };
}

pub fn encode_registry_id(id: u16, w: impl Write) -> Result<()> {
    VarInt(i32::from(id)).encode(w)
}

pub fn decode_registry_id(r: &mut &[u8]) -> Result<u16> {
    let VarInt(raw) = VarInt::decode(r)?;
    u16::try_from(raw)
        .ok()
        .with_context(|| format!("registry id {raw} is outside 0..=65535"))
}

/// A holder writes a registry id one above its number, and zero for an inline value.
pub fn encode_holder_id(reference: Option<u16>, w: impl Write) -> Result<()> {
    VarInt(reference.map_or(0, |id| i32::from(id).saturating_add(1))).encode(w)
}

pub fn decode_holder_id(r: &mut &[u8]) -> Result<Option<u16>> {
    let VarInt(raw) = VarInt::decode(r)?;
    if raw == 0 {
        return Ok(None);
    }
    raw.checked_sub(1)
        .and_then(|id| u16::try_from(id).ok())
        .map(Some)
        .with_context(|| format!("holder id {raw} is outside 0..=65536"))
}

impl<R> Encode for WireId<R> {
    fn encode(&self, w: impl Write) -> Result<()> {
        encode_registry_id(self.number, w)
    }
}

impl<R> Decode<'_> for WireId<R> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        decode_registry_id(r).map(WireId::new)
    }
}

impl<R: Registered, V: Encode> Encode for Ref<Wire, R, V> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        match self {
            Ref::Id(id) => encode_holder_id(Some(id.number()), w),
            Ref::Inline(value) => {
                encode_holder_id(None, &mut w)?;
                value.encode(w)
            }
        }
    }
}

impl<'a, R: Registered, V: Decode<'a>> Decode<'a> for Ref<Wire, R, V> {
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        Ok(match decode_holder_id(r)? {
            Some(id) => Ref::Id(WireId::new(id)),
            None => Ref::Inline(Arc::new(V::decode(r)?)),
        })
    }
}

impl<R> Encode for WireSet<R> {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        match self {
            WireSet::Tag(tag) => {
                VarInt(0).encode(&mut w)?;
                tag.location().encode(w)
            }
            WireSet::List(ids) => {
                let len = ids.len().saturating_add(1);
                let max = i32::MAX as usize;
                let len = i32::try_from(len).map_err(|_| ErrorKind::TooLong { len, max })?;
                VarInt(len).encode(&mut w)?;
                for id in ids.iter() {
                    id.encode(&mut w)?;
                }
                Ok(())
            }
        }
    }
}

impl<R> Decode<'_> for WireSet<R> {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let raw = VarInt::decode(r)?.0;
        let Ok(raw) = usize::try_from(raw) else {
            return Err(ErrorKind::NegativeLength(raw).into());
        };
        if raw == 0 {
            return Ok(WireSet::Tag(TagKeyBuf::from_location(
                ResourceLocationBuf::decode(r)?,
            )));
        }
        let len = raw.saturating_sub(1);
        let mut ids = Vec::with_capacity(len.min(r.len()));
        for _ in 0..len {
            ids.push(WireId::decode(r)?);
        }
        Ok(WireSet::List(ids.into()))
    }
}
