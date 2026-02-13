use crate::{FixedArray, VarInt, VarLong};
use bitfield_struct::bitfield;
use mcrs_nbt::compound::NbtCompound;
use mcrs_protocol_macros::{Decode, Encode};
use std::borrow::Cow;
use std::io::Write;

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct ChunkData<'a> {
    pub heightmaps: Vec<(VarInt, Cow<'a, [u64]>)>,
    pub data: &'a [u8],
    pub block_entities: Cow<'a, [ChunkDataBlockEntity<'a>]>,
}

const BLOCKS_AND_BIOMES: [u8; 2000] = [0x80; 2000];

impl<'a> Default for ChunkData<'a> {
    fn default() -> Self {
        Self {
            heightmaps: vec![],
            data: BLOCKS_AND_BIOMES.as_slice(),
            block_entities: Cow::Borrowed(&[]),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct LightData<'a> {
    pub sky_light_mask: Cow<'a, [u64]>,
    pub block_light_mask: Cow<'a, [u64]>,
    pub empty_sky_light_mask: Cow<'a, [u64]>,
    pub empty_block_light_mask: Cow<'a, [u64]>,
    pub sky_light_arrays: Cow<'a, [FixedArray<u8, 2048>]>,
    pub block_light_arrays: Cow<'a, [FixedArray<u8, 2048>]>,
}

#[allow(clippy::large_const_arrays)]
const SKY_LIGHT_ARRAYS: [FixedArray<u8, 2048>; 26] = [FixedArray([0xff; 2048]); 26];

impl<'a> Default for LightData<'a> {
    fn default() -> Self {
        Self {
            sky_light_mask: Cow::Borrowed(&[]),
            block_light_mask: Cow::Borrowed(&[]),
            empty_sky_light_mask: Cow::Borrowed(&[]),
            empty_block_light_mask: Cow::Borrowed(&[]),
            sky_light_arrays: Cow::Borrowed(SKY_LIGHT_ARRAYS.as_slice()),
            block_light_arrays: Cow::Borrowed(&[]),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct ChunkDataBlockEntity<'a> {
    pub packed_xz: i8,
    pub y: i16,
    pub kind: VarInt,
    pub data: Cow<'a, NbtCompound>,
}

#[bitfield(u64)]
#[derive(PartialEq, Eq)]
pub struct ChunkBlockUpdateEntry {
    #[bits(4)]
    pub off_y: u8,
    #[bits(4)]
    pub off_z: u8,
    #[bits(4)]
    pub off_x: u8,
    pub block_state: u16,
    #[bits(36)]
    _pad: u64,
}

impl crate::Encode for ChunkBlockUpdateEntry {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarLong(self.0 as _).encode(w)
    }
}

impl crate::Decode<'_> for ChunkBlockUpdateEntry {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(ChunkBlockUpdateEntry(VarLong::decode(r)?.0 as _))
    }
}

pub enum Palette<V> {
    Single(V),
    Indirect(Box<[V]>),
    Direct,
}

pub struct PalettedContainer<V> {
    pub bits_per_entry: u8,
    pub palette: Palette<V>,
    pub packed_data: Box<[i64]>,
}

impl<V: Into<VarInt> + Copy> mcrs_protocol::Encode for PalettedContainer<V> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.bits_per_entry.encode(&mut w)?;

        match &self.palette {
            Palette::Single(id) => {
                (*id).into().encode(&mut w)?;
            }
            Palette::Indirect(palette) => {
                VarInt(palette.len() as i32).encode(&mut w)?;
                for id in palette {
                    (*id)
                        .into()
                        .encode(&mut w)
                        .expect("Failed to encode palette entry");
                }
            }
            Palette::Direct => {}
        }
        self.packed_data.iter().for_each(|v| {
            (*v).encode(&mut w)
                .expect("Failed to encode packed data entry");
        });
        Ok(())
    }
}
