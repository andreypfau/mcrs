use std::cell::Cell;
use std::io::Write;
use std::sync::Arc;

use anyhow::{bail, ensure};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_core::{Id, Registered, ResourceKey, ResourceLocation, TagId, TagKey};
use mcrs_minecraft_item::component::WireRef;
use mcrs_minecraft_particle::WireBlockState;
use mcrs_minecraft_registries::{
    BlockStates, Namer, Ref, Registries, ResolveError, Resolver, Wire, WireId, WireSet,
};

use crate::registry::{decode_holder_id, decode_registry_id, encode_holder_id, encode_registry_id};
use crate::{Decode, Encode, VarInt};

/// What a wire value is checked against. Both ends of a connection number a registry alike: a
/// client holds the registries its server sent, in the order sent.
#[derive(Clone, Copy)]
pub struct WireLookup<'a> {
    registries: &'a Registries,
    blocks: Option<&'a BlockDefinitions>,
}

impl<'a> WireLookup<'a> {
    pub fn new(registries: &'a Registries) -> Self {
        WireLookup {
            registries,
            blocks: None,
        }
    }

    /// Block states cross the wire by state id, which only the block definitions number.
    pub fn with_blocks(mut self, blocks: &'a BlockDefinitions) -> Self {
        self.blocks = Some(blocks);
        self
    }
}

impl Resolver for WireLookup<'_> {
    fn id<R: Registered>(&self, key: &ResourceKey<R>) -> Result<Id<R>, ResolveError> {
        self.registries.id(key)
    }

    fn tag<R: Registered>(&self, tag: &TagKey<R>) -> Result<TagId<R>, ResolveError> {
        self.registries.tag(tag)
    }
}

impl Namer for WireLookup<'_> {
    fn name<R: Registered>(&self, id: Id<R>) -> Option<&ResourceLocation> {
        Namer::name(self.registries, id)
    }

    fn tag_name<R: Registered>(&self, tag: TagId<R>) -> Option<&ResourceLocation> {
        self.registries.tag_name(tag)
    }
}

impl WireLookup<'_> {
    fn blocks(&self) -> Result<&BlockDefinitions, ResolveError> {
        self.blocks
            .ok_or_else(|| ResolveError::new("a block state needs the block definitions"))
    }
}

impl BlockStates for WireLookup<'_> {
    fn block_state(
        &self,
        id: u16,
    ) -> Result<(ResourceLocation, Vec<(String, String)>), ResolveError> {
        self.blocks()?
            .properties_of(id)
            .ok_or_else(|| ResolveError::new(format!("no block state has id {id}")))
    }

    fn block_state_id(
        &self,
        block: &ResourceLocation,
        properties: &[(&str, &str)],
    ) -> Result<u16, ResolveError> {
        self.blocks()?
            .state_id_of(block, properties)
            .ok_or_else(|| ResolveError::new(format!("{block} has no block state {properties:?}")))
    }
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

impl<R> Encode for WireId<R> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(self.number(), w)
    }
}

impl<R> Decode<'_> for WireId<R> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_registry_id(r).map(WireId::new)
    }
}

impl<R: Registered, V: Encode> Encode for Ref<Wire, R, V> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
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
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(match decode_holder_id(r)? {
            Some(id) => Ref::Id(WireId::new(id)),
            None => Ref::Inline(Arc::new(V::decode(r)?)),
        })
    }
}

impl<R: Registered, V: Encode> Encode for WireRef<Wire, R, V> {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        self.0.encode(w)
    }
}

impl<'a, R: Registered, V: Decode<'a>> Decode<'a> for WireRef<Wire, R, V> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ref::decode(r).map(WireRef)
    }
}

impl<R> Encode for WireSet<R> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        match self {
            WireSet::Tag(tag) => {
                VarInt(0).encode(&mut w)?;
                tag.location().encode(w)
            }
            WireSet::List(ids) => {
                VarInt(ids.len() as i32 + 1).encode(&mut w)?;
                for id in ids.iter() {
                    id.encode(&mut w)?;
                }
                Ok(())
            }
        }
    }
}

impl<R> Decode<'_> for WireSet<R> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let raw = VarInt::decode(r)?.0;
        ensure!(raw >= 0, "holder set with negative length");
        if raw == 0 {
            return Ok(WireSet::Tag(TagKey::from_location(
                ResourceLocation::decode(r)?,
            )));
        }
        let len = raw as usize - 1;
        let mut ids = Vec::with_capacity(len.min(r.len()));
        for _ in 0..len {
            ids.push(WireId::decode(r)?);
        }
        Ok(WireSet::List(ids.into()))
    }
}

impl Encode for WireBlockState {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        encode_registry_id(self.0, w)
    }
}

impl Decode<'_> for WireBlockState {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        decode_registry_id(r).map(WireBlockState)
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
