use crate::section::{NetworkSectionKind, SectionValue};
use crate::{BlockStateId, Decode as DecodeTrait, Encode as EncodeTrait, VarInt, VarLong};
use anyhow::{Context, ensure};
use bitfield_struct::bitfield;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol_macros::{Decode, Encode};
use mcrs_voxel_storage::{SectionKind, packed_len};
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

/// A single 2048-byte light nibble payload (4 bits per block × 4096 blocks)
/// for one 16×16×16 chunk.
///
/// Encoded on the wire as `VarInt(2048) + 2048 bytes` to match vanilla's
/// `ByteBufCodecs.byteArray(2048)` codec used inside `ClientboundLightUpdatePacketData`.
/// We keep it as a newtype so a `Cow<'_, [LightChunk]>` keeps zero-copy semantics
/// while the per-element prefix is emitted automatically.
///
/// The vanilla Minecraft protocol documentation (wiki.vg
/// `ClientboundLevelChunkWithLight`) calls this structure a "light section";
/// the Rust identifier uses `LightChunk` to align with the engine's
/// Spout-style chunk vocabulary.
#[derive(Clone, Copy)]
pub struct LightChunk(pub [u8; 2048]);

impl LightChunk {
    pub const ZERO: LightChunk = LightChunk([0u8; 2048]);

    pub const fn new(bytes: [u8; 2048]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 2048] {
        &self.0
    }
}

impl Default for LightChunk {
    fn default() -> Self {
        LightChunk::ZERO
    }
}

impl PartialEq for LightChunk {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}

impl Eq for LightChunk {}

impl std::fmt::Debug for LightChunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LightChunk")
            .field("len", &self.0.len())
            .finish()
    }
}

impl std::ops::Deref for LightChunk {
    type Target = [u8; 2048];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<[u8; 2048]> for LightChunk {
    fn from(value: [u8; 2048]) -> Self {
        Self(value)
    }
}

impl EncodeTrait for LightChunk {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(2048).encode(&mut w)?;
        w.write_all(&self.0)?;
        Ok(())
    }
}

impl<'a> DecodeTrait<'a> for LightChunk {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(len == 2048, "expected light section length 2048, got {len}");
        ensure!(
            r.len() >= 2048,
            "not enough data to decode light section (need 2048, have {})",
            r.len()
        );
        let mut bytes = [0u8; 2048];
        bytes.copy_from_slice(&r[..2048]);
        *r = &r[2048..];
        Ok(LightChunk(bytes))
    }
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct LightData<'a> {
    pub sky_light_mask: Cow<'a, [u64]>,
    pub block_light_mask: Cow<'a, [u64]>,
    pub empty_sky_light_mask: Cow<'a, [u64]>,
    pub empty_block_light_mask: Cow<'a, [u64]>,
    pub sky_light_arrays: Cow<'a, [LightChunk]>,
    pub block_light_arrays: Cow<'a, [LightChunk]>,
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Palette<V> {
    Single(V),
    Indirect(Box<[V]>),
    Direct,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PalettedContainer<V> {
    pub bits_per_entry: u8,
    pub palette: Palette<V>,
    pub packed_data: Box<[i64]>,
}

impl<V: Into<VarInt> + Copy> mcrs_minecraft_protocol::Encode for PalettedContainer<V> {
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

impl<'a, V: SectionValue> DecodeTrait<'a> for PalettedContainer<V> {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let bits_per_entry = u8::decode(r)?;
        let storage_bits = V::Section::wire_storage_bits(bits_per_entry);
        let palette = match bits_per_entry as u32 {
            0 => Palette::Single(read_id(r)?),
            bits if bits <= V::Section::MAX_INDIRECT_BITS => {
                let len = VarInt::decode(r)?.0;
                let len = usize::try_from(len)
                    .with_context(|| format!("negative palette length {len}"))?;
                ensure!(
                    len <= 1 << storage_bits,
                    "a palette of {len} entries is wider than the {storage_bits} bits \
                     the entries are stored at"
                );
                let mut entries = Vec::with_capacity(len);
                for index in 0..len {
                    entries.push(read_id(r).with_context(|| format!("palette entry {index}"))?);
                }
                Palette::Indirect(entries.into_boxed_slice())
            }
            _ => Palette::Direct,
        };

        let words = match storage_bits {
            0 => 0,
            bits => packed_len(bits, V::Section::ENTRY_COUNT),
        };
        ensure!(
            r.len() >= words * 8,
            "container of {bits_per_entry} bits per entry needs {words} packed longs, \
             but only {} bytes remain",
            r.len()
        );
        let mut packed_data = Vec::with_capacity(words);
        for _ in 0..words {
            packed_data.push(i64::decode(r)?);
        }

        Ok(Self {
            bits_per_entry,
            palette,
            packed_data: packed_data.into_boxed_slice(),
        })
    }
}

fn read_id<V: SectionValue>(r: &mut &[u8]) -> anyhow::Result<V> {
    V::from_registry_id(VarInt::decode(r)?.0)
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct ChunkSection {
    pub non_empty_block_count: u16,
    pub fluid_count: u16,
    pub blocks: PalettedContainer<BlockStateId>,
    pub biomes: PalettedContainer<u8>,
}

impl<'a> ChunkData<'a> {
    /// The column's sections, in wire order from the dimension's lowest section
    /// upwards. `section_count` comes from the dimension's height: the blob
    /// carries no count of its own and nothing in it can recover one.
    pub fn sections(&self, section_count: usize) -> anyhow::Result<Vec<ChunkSection>> {
        let mut r = self.data;
        let sections = (0..section_count)
            .map(|index| {
                ChunkSection::decode(&mut r).with_context(|| format!("chunk section {index}"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        ensure!(
            r.is_empty(),
            "{} bytes left over after {section_count} chunk sections",
            r.len()
        );
        Ok(sections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Encode;
    use std::borrow::Cow;

    /// Vanilla `ClientboundLightUpdatePacketData` uses
    /// `ByteBufCodecs.byteArray(2048)` for each light section, which writes
    /// `VarInt(len) + bytes`. Each section therefore has to land on the wire as
    /// `0x80 0x10` (VarInt(2048)) followed by 2048 raw bytes.
    #[test]
    fn light_data_emits_inner_varint_prefix_per_section() {
        let data = LightData {
            sky_light_mask: Cow::Owned(vec![1u64]),
            block_light_mask: Cow::Borrowed(&[]),
            empty_sky_light_mask: Cow::Borrowed(&[]),
            empty_block_light_mask: Cow::Borrowed(&[]),
            sky_light_arrays: Cow::Owned(vec![LightChunk([0xABu8; 2048])]),
            block_light_arrays: Cow::Borrowed(&[]),
        };
        let mut buf = Vec::new();
        data.encode(&mut buf).expect("encode LightData");

        let needle = [0xABu8; 2048];
        let pos = buf
            .windows(2048)
            .position(|w| w == needle)
            .expect("2048 contiguous 0xAB bytes present in encoded LightData");

        // Expect: [outer_len=0x01][inner_len_varint=0x80 0x10][0xAB ... ].
        assert!(
            pos >= 3,
            "not enough preceding bytes for outer+inner prefix"
        );
        assert_eq!(buf[pos - 2], 0x80, "first VarInt(2048) byte must be 0x80");
        assert_eq!(buf[pos - 1], 0x10, "second VarInt(2048) byte must be 0x10");
        assert_eq!(
            buf[pos - 3],
            0x01,
            "outer slice length-prefix must be VarInt(1) = 0x01"
        );

        let exact_run_len = buf[pos..].iter().take_while(|&&b| b == 0xAB).count();
        assert_eq!(
            exact_run_len, 2048,
            "expected exactly 2048 contiguous 0xAB bytes, got {}",
            exact_run_len
        );
    }
}
