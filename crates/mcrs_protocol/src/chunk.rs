use crate::{Decode as DecodeTrait, Encode as EncodeTrait, VarInt, VarLong};
use anyhow::ensure;
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

/// A single 2048-byte light nibble payload (4 bits per block × 4096 blocks).
///
/// Encoded on the wire as `VarInt(2048) + 2048 bytes` to match vanilla's
/// `ByteBufCodecs.byteArray(2048)` codec used inside `ClientboundLightUpdatePacketData`.
/// We keep it as a newtype so a `Cow<'_, [LightSection]>` keeps zero-copy semantics
/// while the per-element prefix is emitted automatically.
#[derive(Clone, Copy)]
pub struct LightSection(pub [u8; 2048]);

impl LightSection {
    pub const ZERO: LightSection = LightSection([0u8; 2048]);

    pub const fn new(bytes: [u8; 2048]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 2048] {
        &self.0
    }
}

impl Default for LightSection {
    fn default() -> Self {
        LightSection::ZERO
    }
}

impl PartialEq for LightSection {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}

impl Eq for LightSection {}

impl std::fmt::Debug for LightSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LightSection")
            .field("len", &self.0.len())
            .finish()
    }
}

impl std::ops::Deref for LightSection {
    type Target = [u8; 2048];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<[u8; 2048]> for LightSection {
    fn from(value: [u8; 2048]) -> Self {
        Self(value)
    }
}

impl EncodeTrait for LightSection {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        VarInt(2048).encode(&mut w)?;
        w.write_all(&self.0)?;
        Ok(())
    }
}

impl<'a> DecodeTrait<'a> for LightSection {
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(
            len == 2048,
            "expected light section length 2048, got {len}"
        );
        ensure!(
            r.len() >= 2048,
            "not enough data to decode light section (need 2048, have {})",
            r.len()
        );
        let mut bytes = [0u8; 2048];
        bytes.copy_from_slice(&r[..2048]);
        *r = &r[2048..];
        Ok(LightSection(bytes))
    }
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct LightData<'a> {
    pub sky_light_mask: Cow<'a, [u64]>,
    pub block_light_mask: Cow<'a, [u64]>,
    pub empty_sky_light_mask: Cow<'a, [u64]>,
    pub empty_block_light_mask: Cow<'a, [u64]>,
    pub sky_light_arrays: Cow<'a, [LightSection]>,
    pub block_light_arrays: Cow<'a, [LightSection]>,
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
            sky_light_arrays: Cow::Owned(vec![LightSection([0xABu8; 2048])]),
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
        assert!(pos >= 3, "not enough preceding bytes for outer+inner prefix");
        assert_eq!(buf[pos - 2], 0x80, "first VarInt(2048) byte must be 0x80");
        assert_eq!(
            buf[pos - 1],
            0x10,
            "second VarInt(2048) byte must be 0x10"
        );
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
