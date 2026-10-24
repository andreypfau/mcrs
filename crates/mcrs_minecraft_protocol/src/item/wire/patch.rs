use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_registries::Wire;

use crate::item::kind::ItemComponentValue;
use crate::item::patch::{ComponentMap, ComponentPatch};
use crate::item::wire::decode_component_value;
use crate::{Decode, Encode, VarInt};
use mcrs_minecraft_item::keys::DataComponentType;

fn encode_with(
    patch: &ComponentPatch<Wire>,
    mut w: impl Write,
    mut value: impl FnMut(&ItemComponentValue<Wire>, &mut dyn Write) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    VarInt(patch.added.len() as i32).encode(&mut w)?;
    VarInt(patch.removed.len() as i32).encode(&mut w)?;
    for added in &patch.added {
        added.kind().encode(&mut w)?;
        value(added, &mut w)?;
    }
    for removed in &patch.removed {
        removed.encode(&mut w)?;
    }
    Ok(())
}

fn decode_with<'a>(
    r: &mut &'a [u8],
    mut value: impl FnMut(DataComponentType, &mut &'a [u8]) -> anyhow::Result<ItemComponentValue<Wire>>,
) -> anyhow::Result<ComponentPatch<Wire>> {
    let added = VarInt::decode(r)?.0;
    let removed = VarInt::decode(r)?.0;
    ensure!(added >= 0 && removed >= 0, "negative component count");
    let mut patch = ComponentPatch::default();
    for _ in 0..added {
        let kind = DataComponentType::decode(r)?;
        patch.set_value(value(kind, r)?);
    }
    for _ in 0..removed {
        patch.remove(DataComponentType::decode(r)?);
    }
    Ok(patch)
}

/// Every added value behind its byte length.
pub fn encode_delimited_patch(patch: &ComponentPatch<Wire>, w: impl Write) -> anyhow::Result<()> {
    encode_with(patch, w, |value, w| {
        let mut bytes = Vec::new();
        value.encode(&mut bytes)?;
        VarInt(bytes.len() as i32).encode(&mut *w)?;
        Ok(w.write_all(&bytes)?)
    })
}

/// The bytes a value leaves unread inside its length prefix are skipped:
/// the declared size is advanced unconditionally.
pub fn decode_delimited_patch(r: &mut &[u8]) -> anyhow::Result<ComponentPatch<Wire>> {
    decode_with(r, |kind, r| {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "negative component length");
        let len = len as usize;
        ensure!(
            len <= r.len(),
            "component {} declares {len} bytes but {} remain",
            kind.location(),
            r.len()
        );
        let (mut slice, rest) = r.split_at(len);
        let value = decode_component_value(kind, &mut slice)?;
        *r = rest;
        Ok(value)
    })
}

impl Encode for ComponentPatch<Wire> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_with(self, w, |value, w| value.encode(w))
    }
}

impl Decode<'_> for ComponentPatch<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_with(r, decode_component_value)
    }
}

impl Encode for ComponentMap<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let w: &mut dyn Write = &mut w;
        VarInt(self.0.len() as i32).encode(&mut *w)?;
        for value in &self.0 {
            value.kind().encode(&mut *w)?;
            value.encode(&mut *w)?;
        }
        Ok(())
    }
}

impl Decode<'_> for ComponentMap<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len >= 0, "attempt to decode a list with negative length");
        let mut map = ComponentMap::default();
        for _ in 0..len {
            let kind = DataComponentType::decode(r)?;
            map.set_value(decode_component_value(kind, r)?);
        }
        Ok(map)
    }
}
