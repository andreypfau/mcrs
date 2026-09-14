use crate::section::{Biomes, Blocks, NetworkSectionKind, PaletteForm, SectionValue};
use crate::{Decode as DecodeTrait, Encode as EncodeTrait, VarInt, VarLong};
use anyhow::{Context, bail, ensure};
use bitfield_struct::bitfield;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol_macros::{Decode, Encode};
use mcrs_voxel_storage::{
    PalettedContainer, SectionKind, VoxelId, any_entry_past, first_entry_past, pack_from,
    packed_len, remap_into, unpack_into,
};
use std::borrow::Cow;
use std::hash::Hash;
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

/// The network form drops the palette list past `MAX_INDIRECT_BITS` and packs
/// registry ids directly; the save keeps a palette at every size.
impl<V: SectionValue + Hash + Eq + Default, const DIM: usize> EncodeTrait
    for PalettedContainer<V, DIM>
{
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        const { assert!(DIM == V::Section::SIZE) };
        let data = match self {
            PalettedContainer::Homogeneous(value) => {
                0u8.encode(&mut w)?;
                return (*value).into().encode(w);
            }
            PalettedContainer::Heterogeneous(data) => data,
        };
        let packed = match V::Section::network_form(data.palette.len()) {
            PaletteForm::Single => unreachable!("a heterogeneous container holds two values"),
            PaletteForm::Indirect { bits } => {
                (bits as u8).encode(&mut w)?;
                let (palette, packed) = self.to_palette_and_packed_data(bits as u8);
                VarInt(palette.len() as i32).encode(&mut w)?;
                for value in palette.iter() {
                    (*value).into().encode(&mut w)?;
                }
                packed
            }
            PaletteForm::Direct { bits } => {
                (bits as u8).encode(&mut w)?;
                let cells = data.cube.as_flattened().as_flattened();
                pack_from(bits, cells, |&value| {
                    let id: VarInt = value.into();
                    id.0 as u32
                })
            }
        };
        for word in packed.iter() {
            word.encode(&mut w)?;
        }
        Ok(())
    }
}

impl<'a, V: SectionValue + Hash + Eq + Default, const DIM: usize> DecodeTrait<'a>
    for PalettedContainer<V, DIM>
{
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        const { assert!(DIM == V::Section::SIZE) };
        let bits_per_entry = u8::decode(r)?;
        if bits_per_entry == 0 {
            return Ok(Self::Homogeneous(read_id(r)?));
        }
        let storage_bits = V::Section::wire_storage_bits(bits_per_entry);
        let palette = if bits_per_entry as u32 <= V::Section::MAX_INDIRECT_BITS {
            let len = VarInt::decode(r)?.0;
            let len =
                usize::try_from(len).with_context(|| format!("negative palette length {len}"))?;
            ensure!(
                len <= 1 << storage_bits,
                "a palette of {len} entries is wider than the {storage_bits} bits \
                 the entries are stored at"
            );
            let mut entries = Vec::with_capacity(len);
            for index in 0..len {
                entries.push(read_id(r).with_context(|| format!("palette entry {index}"))?);
            }
            Some(entries)
        } else {
            None
        };

        let entry_count = V::Section::ENTRY_COUNT;
        let words = packed_len(storage_bits, entry_count);
        ensure!(
            r.len() >= words * 8,
            "container of {bits_per_entry} bits per entry needs {words} packed longs, \
             but only {} bytes remain",
            r.len()
        );
        let packed = (0..words)
            .map(|_| i64::decode(r))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let mut cells = vec![V::default(); entry_count];
        match palette {
            Some(entries) => {
                if any_entry_past(storage_bits, &packed, entry_count, entries.len()) {
                    let index = first_entry_past(storage_bits, &packed, entry_count, entries.len())
                        .expect("an entry is past the palette");
                    bail!("palette index {index} of {}", entries.len());
                }
                remap_into(storage_bits, &packed, &entries, &mut cells)
                    .expect("the packed length was read to fit");
            }
            None => {
                let mut ids = vec![0u16; entry_count];
                unpack_into(storage_bits, &packed, &mut ids)
                    .expect("the packed length was read to fit");
                for (cell, id) in cells.iter_mut().zip(ids) {
                    *cell = V::from_registry_id(id as i32)?;
                }
            }
        }
        Ok(Self::from_cells(&cells))
    }
}

fn read_id<V: SectionValue>(r: &mut &[u8]) -> anyhow::Result<V> {
    V::from_registry_id(VarInt::decode(r)?.0)
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct ChunkSection {
    pub non_empty_block_count: u16,
    pub fluid_count: u16,
    pub blocks: PalettedContainer<VoxelId, { Blocks::SIZE }>,
    pub biomes: PalettedContainer<u8, { Biomes::SIZE }>,
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
