use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registries::{FromWire, Named, Namer, Resolve, Wire, WireId};

use crate::item::lookup::{WireLookup, nested};
use crate::item::patch::ComponentPatch;
use crate::item::stack::{
    HashedPatchMap, ItemStackValue, MAX_HASHED_COMPONENTS, ProtoStack, Template,
};
use crate::item::wire::{decode_delimited_patch, encode_delimited_patch};
use crate::{Decode, Encode, VarInt};
use mcrs_minecraft_item::keys::DataComponentType;

impl Encode for Template<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.0.item.encode(&mut w)?;
        VarInt(self.0.count.0).encode(&mut w)?;
        self.0.components.encode(w)
    }
}

impl Decode<'_> for Template<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        nested(|| {
            let item = WireId::<Item>::decode(r)?;
            let count = VarInt::decode(r)?.0;
            let components = ComponentPatch::decode(r)?;
            Template::new(item, count, components).map_err(anyhow::Error::msg)
        })
    }
}

fn encode_with(
    stack: &ProtoStack<Wire>,
    mut w: impl Write,
    patch: impl FnOnce(&ComponentPatch<Wire>, &mut dyn Write) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    if stack.is_empty() {
        return VarInt(0).encode(w);
    }
    VarInt(stack.count).encode(&mut w)?;
    stack.id.encode(&mut w)?;
    patch(&stack.components, &mut w)
}

fn decode_with<'a>(
    r: &mut &'a [u8],
    patch: impl FnOnce(&mut &'a [u8]) -> anyhow::Result<ComponentPatch<Wire>>,
) -> anyhow::Result<ProtoStack<Wire>> {
    let count = VarInt::decode(r)?.0;
    if count <= 0 {
        return Ok(ProtoStack::default());
    }
    let stack = ProtoStack {
        id: WireId::<Item>::decode(r)?,
        count,
        components: patch(r)?,
    };
    Ok(if stack.is_empty() {
        ProtoStack::default()
    } else {
        stack
    })
}

impl Encode for ProtoStack<Wire> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_with(self, w, |patch, w| patch.encode(w))
    }
}

impl Decode<'_> for ProtoStack<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_with(r, ComponentPatch::decode)
    }
}

/// A stack whose component values carry a length prefix.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DelimitedStack(pub ProtoStack<Wire>);

impl DelimitedStack {
    /// Vanilla validates by re-encoding through the persistent codec; here every
    /// codec's checks sit on the read side, so the stack is read back from its
    /// persistent form instead.
    pub fn check(self, lookup: &WireLookup<'_>) -> anyhow::Result<ProtoStack> {
        let stack = ProtoStack::from_wire(self.0, lookup)?;
        if !stack.is_empty() {
            let value = stack.to_value()?.name(lookup)?;
            let persistent = mcrs_minecraft_nbt::to_nbt_compound(&value)?;
            mcrs_minecraft_nbt::from_tag::<ItemStackValue<Named>>(persistent.into())?;
        }
        Ok(stack)
    }
}

impl Encode for DelimitedStack {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_with(&self.0, w, |patch, w| encode_delimited_patch(patch, w))
    }
}

impl Decode<'_> for DelimitedStack {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_with(r, decode_delimited_patch).map(DelimitedStack)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashedStack {
    pub id: Id<Item>,
    pub count: i32,
    pub components: HashedPatchMap,
}

impl HashedStack {
    pub fn create(stack: &ProtoStack, names: &impl Namer) -> anyhow::Result<Option<Self>> {
        if stack.is_empty() {
            return Ok(None);
        }
        Ok(Some(HashedStack {
            id: stack.id,
            count: stack.count,
            components: HashedPatchMap::create(&stack.components, names)?,
        }))
    }

    pub fn matches(&self, stack: &ProtoStack, names: &impl Namer) -> bool {
        self.count == stack.count
            && self.id == stack.id
            && self.components.matches(&stack.components, names)
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
            id: Id::<Item>::decode(r)?,
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
            let kind = DataComponentType::decode(r)?;
            let hash = i32::decode(r)?;
            match map.added.iter_mut().find(|(k, _)| *k == kind) {
                Some(entry) => entry.1 = hash,
                None => map.added.push((kind, hash)),
            }
        }
        let removed = count(r)?;
        for _ in 0..removed {
            let kind = DataComponentType::decode(r)?;
            if !map.removed.contains(&kind) {
                map.removed.push(kind);
            }
        }
        Ok(map)
    }
}
