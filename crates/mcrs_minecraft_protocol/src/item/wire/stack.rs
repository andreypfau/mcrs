use std::io::Write;

use anyhow::{Context, ensure};
use bytes::Bytes;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::codec::{self, Validate};
use mcrs_minecraft_registry::{ItemId, ItemReg, RegistryLookup, RegistryName};

use crate::item::ctx::{DecodeCtx, EncodeCtx, Opaque, nested};
use crate::item::kind::ItemComponentKind;
use crate::item::patch::ComponentPatch;
use crate::item::stack::{HashedPatchMap, ItemStackValue, MAX_HASHED_COMPONENTS, Template};
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


#[derive(Clone, Debug, PartialEq, Default)]
pub struct ProtoStack {
    pub id: ItemId,
    pub count: i32,
    pub components: ComponentPatch,
}

impl ProtoStack {
    pub const EMPTY: ProtoStack = ProtoStack {
        id: ItemId(0),
        count: 0,
        components: ComponentPatch::EMPTY,
    };

    #[must_use]
    pub const fn new(item: ItemId, count: i32, components: ComponentPatch) -> Self {
        Self {
            id: item,
            count,
            components,
        }
    }

    #[must_use]
    pub const fn with_count(mut self, count: i32) -> Self {
        self.count = count;
        self
    }

    #[must_use]
    pub const fn with_item(mut self, item: ItemId) -> Self {
        self.id = item;
        self
    }

    /// No items, or the air item.
    pub const fn is_empty(&self) -> bool {
        self.count <= 0 || self.id.0 == 0
    }

    pub fn from_value(value: &ItemStackValue, ctx: &dyn RegistryLookup) -> anyhow::Result<Self> {
        let id = ctx
            .id(ItemReg::NAME, value.item.location())
            .with_context(|| format!("{} is not in registry item", value.item))?;
        Ok(ProtoStack {
            id: ItemId(u16::try_from(id).with_context(|| format!("item id {id} is out of range"))?),
            count: value.count.0,
            components: value.components.clone(),
        })
    }

    pub fn to_value(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<ItemStackValue> {
        ensure!(!self.is_empty(), "an empty stack has no persistent form");
        let name = ctx
            .name(ItemReg::NAME, self.id.0 as u32)
            .with_context(|| format!("registry item has no id {}", self.id.0))?;
        let value = ItemStackValue {
            item: ResourceKey::from_location(name.clone()),
            count: codec::Bounded(self.count),
            components: self.components.clone(),
        };
        ensure!(
            (1..=99).contains(&self.count),
            "Value must be within range [1;99]: {}",
            self.count
        );
        value.validate().map_err(anyhow::Error::msg)?;
        Ok(value)
    }

    fn encode_with(
        &self,
        ctx: &dyn RegistryLookup,
        mut w: impl Write,
        patch: impl FnOnce(&ComponentPatch, &dyn RegistryLookup, &mut dyn Write) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if self.is_empty() {
            return VarInt(0).encode(w);
        }
        VarInt(self.count).encode(&mut w)?;
        self.id.encode(&mut w)?;
        patch(&self.components, ctx, &mut w)
    }

    fn decode_with<'a>(
        ctx: &dyn RegistryLookup,
        r: &mut &'a [u8],
        patch: impl FnOnce(&dyn RegistryLookup, &mut &'a [u8]) -> anyhow::Result<ComponentPatch>,
    ) -> anyhow::Result<Self> {
        let count = VarInt::decode(r)?.0;
        if count <= 0 {
            return Ok(ProtoStack::EMPTY);
        }
        let stack = ProtoStack {
            id: ItemId::decode(r)?,
            count,
            components: patch(ctx, r)?,
        };
        Ok(if stack.is_empty() { ProtoStack::EMPTY } else { stack })
    }

    pub fn encode_delimited_ctx(
        &self,
        ctx: &dyn RegistryLookup,
        w: impl Write,
    ) -> anyhow::Result<()> {
        self.encode_with(ctx, w, |patch, ctx, w| encode_delimited_patch(patch, ctx, w))
    }

    pub fn decode_delimited_ctx(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        Self::decode_with(ctx, r, decode_delimited_patch)
    }
}

impl EncodeCtx for ProtoStack {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        self.encode_with(ctx, w, |patch, ctx, w| patch.encode_ctx(ctx, w))
    }
}

impl<'a> DecodeCtx<'a> for ProtoStack {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Self::decode_with(ctx, r, ComponentPatch::decode_ctx)
    }
}

/// The exact bytes of one wire stack, kept so packets can
/// carry stacks without the registries; the value walks the layout to find
/// its length and is resolved on demand.
// ponytail: every stack is parsed twice, once to measure and once to resolve; fine at inventory
// sizes, replace with a macro-generated skip when it shows up in a profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawStack(pub Bytes);

impl Default for RawStack {
    fn default() -> Self {
        RawStack::EMPTY
    }
}

impl RawStack {
    pub const EMPTY: RawStack = RawStack(Bytes::from_static(&[0]));

    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<ProtoStack> {
        let mut r = &self.0[..];
        let stack = ProtoStack::decode_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after a stack", r.len());
        Ok(stack)
    }

    pub fn from_stack(stack: &ProtoStack, ctx: &dyn RegistryLookup) -> anyhow::Result<RawStack> {
        let mut bytes = Vec::new();
        stack.encode_ctx(ctx, &mut bytes)?;
        Ok(RawStack(bytes.into()))
    }
}

impl Encode for RawStack {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl Decode<'_> for RawStack {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        ProtoStack::decode_ctx(&Opaque, r)?;
        Ok(RawStack(Bytes::copy_from_slice(
            &start[..start.len() - r.len()],
        )))
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
        let mut r = &self.0[..];
        let stack = ProtoStack::decode_delimited_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after a stack", r.len());
        if !stack.is_empty() {
            let value = stack.to_value(ctx)?;
            let persistent = mcrs_minecraft_nbt::to_nbt_compound(&value)?;
            mcrs_minecraft_nbt::from_tag::<ItemStackValue>(persistent.into())?;
        }
        Ok(stack)
    }

    pub fn from_stack(stack: &ProtoStack, ctx: &dyn RegistryLookup) -> anyhow::Result<RawDelimitedStack> {
        let mut bytes = Vec::new();
        stack.encode_delimited_ctx(ctx, &mut bytes)?;
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
        ProtoStack::decode_delimited_ctx(&Opaque, r)?;
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
        self.count == stack.count && self.id == stack.id && self.components.matches(&stack.components)
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

