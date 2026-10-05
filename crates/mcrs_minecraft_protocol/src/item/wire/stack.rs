use std::io::Write;
use std::marker::PhantomData;

use anyhow::ensure;
use bytes::Bytes;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_registry::{ItemId, RegistryLookup};

use crate::item::ctx::{DecodeCtx, EncodeCtx, Opaque, Raw, nested, scoped};
use crate::item::kind::ItemComponentKind;
use crate::item::patch::ComponentPatch;
use crate::item::stack::{
    HashedPatchMap, ItemStackValue, MAX_HASHED_COMPONENTS, ProtoStack, Template,
};
use crate::item::wire::{decode_delimited_patch, encode_delimited_patch};
use crate::{Decode, Encode, VarInt};

impl EncodeCtx for Template {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.0.item.encode_ctx(ctx, &mut w)?;
        VarInt(self.0.count.0).encode(&mut w)?;
        self.0.components.encode_ctx(ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for Template {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        nested(|| {
            let item = ResourceKey::decode_ctx(ctx, r)?;
            let count = VarInt::decode(r)?.0;
            let components = ComponentPatch::decode_ctx(ctx, r)?;
            Template::new(item, count, components).map_err(anyhow::Error::msg)
        })
    }
}

fn encode_with(
    stack: &ProtoStack,
    ctx: &dyn RegistryLookup,
    mut w: impl Write,
    patch: impl FnOnce(&ComponentPatch, &dyn RegistryLookup, &mut dyn Write) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    if stack.is_empty() {
        return VarInt(0).encode(w);
    }
    VarInt(stack.count).encode(&mut w)?;
    stack.id.encode(&mut w)?;
    patch(&stack.components, ctx, &mut w)
}

fn decode_with<'a>(
    ctx: &dyn RegistryLookup,
    r: &mut &'a [u8],
    patch: impl FnOnce(&dyn RegistryLookup, &mut &'a [u8]) -> anyhow::Result<ComponentPatch>,
) -> anyhow::Result<ProtoStack> {
    let count = VarInt::decode(r)?.0;
    if count <= 0 {
        return Ok(ProtoStack::EMPTY);
    }
    let stack = ProtoStack {
        id: ItemId::decode(r)?,
        count,
        components: patch(ctx, r)?,
    };
    Ok(if stack.is_empty() {
        ProtoStack::EMPTY
    } else {
        stack
    })
}

pub fn encode_delimited_ctx(
    stack: &ProtoStack,
    ctx: &dyn RegistryLookup,
    w: impl Write,
) -> anyhow::Result<()> {
    encode_with(stack, ctx, w, |patch, ctx, w| {
        encode_delimited_patch(patch, ctx, w)
    })
}

pub fn decode_delimited_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<ProtoStack> {
    decode_with(ctx, r, decode_delimited_patch)
}

impl EncodeCtx for ProtoStack {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        encode_with(self, ctx, w, |patch, ctx, w| patch.encode_ctx(ctx, w))
    }
}

impl<'a> DecodeCtx<'a> for ProtoStack {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_with(ctx, r, ComponentPatch::decode_ctx)
    }
}

// chisle: every stack is parsed twice, once to measure and once to resolve; fine at inventory
// sizes, replace with a macro-generated skip when it shows up in a profile.
pub type RawStack = Raw<ProtoStack>;

impl Default for RawStack {
    fn default() -> Self {
        RawStack::EMPTY
    }
}

impl RawStack {
    pub const EMPTY: RawStack = Raw(Bytes::from_static(&[0]), PhantomData);

    pub fn from_stack(stack: &ProtoStack, ctx: &dyn RegistryLookup) -> anyhow::Result<RawStack> {
        Raw::from_value(stack, ctx)
    }
}

/// A stack whose component values carry a length prefix, validated on resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawDelimitedStack(pub Bytes);

impl Default for RawDelimitedStack {
    fn default() -> Self {
        RawDelimitedStack::EMPTY
    }
}

impl RawDelimitedStack {
    pub const EMPTY: RawDelimitedStack = RawDelimitedStack(Bytes::from_static(&[0]));

    /// Vanilla validates by re-encoding through the persistent codec; here every
    /// codec's checks sit on the read side, so the stack is read back from its
    /// persistent form instead.
    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<ProtoStack> {
        scoped(ctx, || {
            let mut r = &self.0[..];
            let stack = decode_delimited_ctx(ctx, &mut r)?;
            ensure!(r.is_empty(), "{} trailing bytes after a stack", r.len());
            if !stack.is_empty() {
                let value = stack.to_value(ctx)?;
                let persistent = mcrs_minecraft_nbt::to_nbt_compound(&value)?;
                mcrs_minecraft_nbt::from_tag::<ItemStackValue>(persistent.into())?;
            }
            Ok(stack)
        })
    }

    pub fn from_stack(
        stack: &ProtoStack,
        ctx: &dyn RegistryLookup,
    ) -> anyhow::Result<RawDelimitedStack> {
        let mut bytes = Vec::new();
        scoped(ctx, || encode_delimited_ctx(stack, ctx, &mut bytes))?;
        Ok(RawDelimitedStack(bytes.into()))
    }
}

impl Encode for RawDelimitedStack {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl Decode<'_> for RawDelimitedStack {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        mcrs_minecraft_registry::skip_sets(|| decode_delimited_ctx(&Opaque, r))?;
        Ok(RawDelimitedStack(Bytes::copy_from_slice(
            &start[..start.len() - r.len()],
        )))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HashedStack {
    pub id: ItemId,
    pub count: i32,
    pub components: HashedPatchMap,
}

impl HashedStack {
    pub fn create(stack: &ProtoStack) -> anyhow::Result<Option<Self>> {
        if stack.is_empty() {
            return Ok(None);
        }
        Ok(Some(HashedStack {
            id: stack.id,
            count: stack.count,
            components: HashedPatchMap::create(&stack.components)?,
        }))
    }

    pub fn matches(&self, stack: &ProtoStack) -> bool {
        self.count == stack.count
            && self.id == stack.id
            && self.components.matches(&stack.components)
    }
}

impl Encode for HashedStack {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.id.encode(&mut w)?;
        VarInt(self.count).encode(&mut w)?;
        self.components.encode(w)
    }
}

impl Decode<'_> for HashedStack {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(HashedStack {
            id: ItemId::decode(r)?,
            count: VarInt::decode(r)?.0,
            components: HashedPatchMap::decode(r)?,
        })
    }
}

impl Encode for HashedPatchMap {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        ensure!(
            self.added.len() <= MAX_HASHED_COMPONENTS
                && self.removed.len() <= MAX_HASHED_COMPONENTS,
            "more than {MAX_HASHED_COMPONENTS} hashed components"
        );
        VarInt(self.added.len() as i32).encode(&mut w)?;
        for (kind, hash) in &self.added {
            kind.encode(&mut w)?;
            hash.encode(&mut w)?;
        }
        VarInt(self.removed.len() as i32).encode(&mut w)?;
        for kind in &self.removed {
            kind.encode(&mut w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for HashedPatchMap {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let count = |r: &mut &[u8]| -> anyhow::Result<usize> {
            let n = VarInt::decode(r)?.0;
            ensure!(
                (0..=MAX_HASHED_COMPONENTS as i32).contains(&n),
                "hashed component count {n} is outside 0..={MAX_HASHED_COMPONENTS}"
            );
            Ok(n as usize)
        };
        let added = count(r)?;
        let mut map = HashedPatchMap::default();
        for _ in 0..added {
            let kind = ItemComponentKind::decode(r)?;
            let hash = i32::decode(r)?;
            match map.added.iter_mut().find(|(k, _)| *k == kind) {
                Some(entry) => entry.1 = hash,
                None => map.added.push((kind, hash)),
            }
        }
        let removed = count(r)?;
        for _ in 0..removed {
            let kind = ItemComponentKind::decode(r)?;
            if !map.removed.contains(&kind) {
                map.removed.push(kind);
            }
        }
        Ok(map)
    }
}
