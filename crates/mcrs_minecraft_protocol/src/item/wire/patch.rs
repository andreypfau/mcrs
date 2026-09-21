use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::kind::{ItemComponentKind, ItemComponentValue};
use crate::item::patch::{ComponentMap, ComponentPatch};
use crate::item::wire::decode_component_value;
use crate::{Decode, Encode, VarInt};

fn encode_with(
    patch: &ComponentPatch,
    ctx: &dyn RegistryLookup,
    mut w: impl Write,
    mut value: impl FnMut(&ItemComponentValue, &dyn RegistryLookup, &mut dyn Write) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    VarInt(patch.added.len() as i32).encode(&mut w)?;
    VarInt(patch.removed.len() as i32).encode(&mut w)?;
    for added in &patch.added {
        added.kind().encode(&mut w)?;
        value(added, ctx, &mut w)?;
    }
    for removed in &patch.removed {
        removed.encode(&mut w)?;
    }
    Ok(())
}

fn decode_with<'a>(
    ctx: &dyn RegistryLookup,
    r: &mut &'a [u8],
    mut value: impl FnMut(
        ItemComponentKind,
        &dyn RegistryLookup,
        &mut &'a [u8],
    ) -> anyhow::Result<ItemComponentValue>,
) -> anyhow::Result<ComponentPatch> {
    let added = VarInt::decode(r)?.0;
    let removed = VarInt::decode(r)?.0;
    ensure!(added >= 0 && removed >= 0, "negative component count");
    let mut patch = ComponentPatch::EMPTY;
    for _ in 0..added {
        let kind = ItemComponentKind::decode(r)?;
        patch.set_value(value(kind, ctx, r)?);
    }
    for _ in 0..removed {
        patch.remove(ItemComponentKind::decode(r)?);
    }
    Ok(patch)
}

/// Every added value behind its byte length.
pub fn encode_delimited_patch(
    patch: &ComponentPatch,
    ctx: &dyn RegistryLookup,
    w: impl Write,
) -> anyhow::Result<()> {
    encode_with(patch, ctx, w, |value, ctx, w| {
        let mut bytes = Vec::new();
        value.encode_ctx(ctx, &mut bytes)?;
        VarInt(bytes.len() as i32).encode(&mut *w)?;
        Ok(w.write_all(&bytes)?)
    })
}

/// The bytes a value leaves unread inside its length prefix are skipped:
/// the declared size is advanced unconditionally.
pub fn decode_delimited_patch(ctx: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<ComponentPatch> {
    decode_with(ctx, r, |kind, ctx, r| {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "negative component length");
        let len = len as usize;
        ensure!(
            len <= r.len(),
            "component {kind} declares {len} bytes but {} remain",
            r.len()
        );
        let (mut slice, rest) = r.split_at(len);
        let value = decode_component_value(kind, ctx, &mut slice)?;
        *r = rest;
        Ok(value)
    })
}

impl EncodeCtx for ComponentPatch {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, w: impl Write) -> anyhow::Result<()> {
        encode_with(self, ctx, w, |value, ctx, w| value.encode_ctx(ctx, w))
    }
}

impl<'a> DecodeCtx<'a> for ComponentPatch {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        decode_with(ctx, r, decode_component_value)
    }
}

impl EncodeCtx for ComponentMap {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        let w: &mut dyn Write = &mut w;
        VarInt(self.0.len() as i32).encode(&mut *w)?;
        for value in &self.0 {
            value.kind().encode(&mut *w)?;
            value.encode_ctx(ctx, &mut *w)?;
        }
        Ok(())
    }
}

impl<'a> DecodeCtx<'a> for ComponentMap {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode a list with negative length");
        let mut map = ComponentMap::default();
        for _ in 0..len {
            let kind = ItemComponentKind::decode(r)?;
            map.set_value(decode_component_value(kind, ctx, r)?);
        }
        Ok(map)
    }
}
