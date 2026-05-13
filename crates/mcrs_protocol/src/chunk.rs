use crate::{VarInt, VarLong};
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
    pub sky_light_arrays: Cow<'a, [[u8; 2048]]>,
    pub block_light_arrays: Cow<'a, [[u8; 2048]]>,
}

impl<'a> Default for LightData<'a> {
    fn default() -> Self {
        Self {
            sky_light_mask: Cow::Borrowed(&[]),
            block_light_mask: Cow::Borrowed(&[]),
            empty_sky_light_mask: Cow::Borrowed(&[]),
            empty_block_light_mask: Cow::Borrowed(&[]),
            sky_light_arrays: Cow::Borrowed(&[]),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Encode;
    use std::borrow::Cow;

    #[test]
    fn light_data_encodes_2048_raw_bytes_with_no_inner_prefix() {
        let data = LightData {
            sky_light_mask: Cow::Owned(vec![1u64]),
            block_light_mask: Cow::Borrowed(&[]),
            empty_sky_light_mask: Cow::Borrowed(&[]),
            empty_block_light_mask: Cow::Borrowed(&[]),
            sky_light_arrays: Cow::Owned(vec![[0xABu8; 2048]]),
            block_light_arrays: Cow::Borrowed(&[]),
        };
        let mut buf = Vec::new();
        data.encode(&mut buf).expect("encode LightData");

        let needle = [0xABu8; 2048];
        let pos = buf
            .windows(2048)
            .position(|w| w == needle)
            .expect("2048 contiguous 0xAB bytes present in encoded LightData");

        assert!(pos > 0, "no preceding bytes; encoding malformed");
        let prefix_byte = buf[pos - 1];
        assert_eq!(
            prefix_byte, 0x01,
            "outer Cow slice length-prefix expected to be VarInt(1) = 0x01, got 0x{:02X}; inner VarInt(2048) prefix would be 0x10 here",
            prefix_byte
        );
        if pos >= 2 {
            let prefix_byte_prev = buf[pos - 2];
            assert_ne!(
                prefix_byte_prev, 0x80,
                "if 0x80 appears at pos-2 followed by 0x10 at pos-1, the inner array is incorrectly length-prefixed"
            );
        }

        let exact_run_len = buf[pos..].iter().take_while(|&&b| b == 0xAB).count();
        assert_eq!(
            exact_run_len, 2048,
            "expected exactly 2048 contiguous 0xAB bytes, got {}",
            exact_run_len
        );
    }
}
