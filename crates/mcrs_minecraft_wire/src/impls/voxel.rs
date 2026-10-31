use std::io::Write;

use mcrs_voxel::{BlockPos, ColumnPos, SectionPos};

use crate::{Context, Decode, Encode, Result};

/// Packs three signed coordinates into one `u64`, `x` in the high bits, each in two's complement
/// of its width.
fn pack(pos: [i32; 3], bits: [u32; 3]) -> Option<u64> {
    let mut packed = 0;
    for (value, width) in pos.into_iter().zip(bits) {
        let limit = 1i64 << (width - 1);
        if !(-limit..limit).contains(&i64::from(value)) {
            return None;
        }
        packed = packed << width | (value as u64 & ((1 << width) - 1));
    }
    Some(packed)
}

fn unpack(packed: u64, bits: [u32; 3]) -> [i32; 3] {
    let mut shift = 64;
    bits.map(|width| {
        shift -= width;
        ((packed << (64 - shift - width)) as i64 >> (64 - width)) as i32
    })
}

/// `x` 26 bits, `z` 26 bits, `y` 12 bits.
const BLOCK_POS_BITS: [u32; 3] = [26, 26, 12];

impl Encode for BlockPos {
    fn encode(&self, w: impl Write) -> Result<()> {
        pack([self.x, self.z, self.y], BLOCK_POS_BITS)
            .with_context(|| format!("block position {self} is out of range"))?
            .encode(w)
    }
}

impl Decode<'_> for BlockPos {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let [x, z, y] = unpack(u64::decode(r)?, BLOCK_POS_BITS);
        Ok(BlockPos::new(x, y, z))
    }
}

/// `x` 22 bits, `z` 22 bits, `y` 20 bits.
const SECTION_POS_BITS: [u32; 3] = [22, 22, 20];

impl Encode for SectionPos {
    fn encode(&self, w: impl Write) -> Result<()> {
        pack([self.x, self.z, self.y], SECTION_POS_BITS)
            .with_context(|| format!("section position {self} is out of range"))?
            .encode(w)
    }
}

impl Decode<'_> for SectionPos {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let [x, z, y] = unpack(u64::decode(r)?, SECTION_POS_BITS);
        Ok(SectionPos::new(x, y, z))
    }
}

impl Encode for ColumnPos {
    fn encode(&self, mut w: impl Write) -> Result<()> {
        self.x.encode(&mut w)?;
        self.z.encode(w)
    }
}

impl Decode<'_> for ColumnPos {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        Ok(ColumnPos {
            x: i32::decode(r)?,
            z: i32::decode(r)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_position_packs_only_within_its_bit_widths() {
        let xzs = [
            (-33554432, true),
            (-33554433, false),
            (33554431, true),
            (33554432, false),
        ];
        let ys = [
            (-2048, true),
            (-2049, false),
            (2047, true),
            (2048, false),
            (-1, true),
        ];
        for (x, x_valid) in xzs {
            for (y, y_valid) in ys {
                for (z, z_valid) in [(0, true), (-1, true), (33554432, false)] {
                    let pos = BlockPos::new(x, y, z);
                    let mut bytes = Vec::new();
                    let encoded = pos.encode(&mut bytes);
                    assert_eq!(encoded.is_ok(), x_valid && y_valid && z_valid, "{pos}");
                    if encoded.is_ok() {
                        assert_eq!(BlockPos::decode(&mut bytes.as_slice()).unwrap(), pos);
                    }
                }
            }
        }
        let mut bytes = Vec::new();
        BlockPos::new(1, -1, 3).encode(&mut bytes).unwrap();
        assert_eq!(
            u64::decode(&mut bytes.as_slice()).unwrap(),
            1 << 38 | 3 << 12 | 0xFFF
        );
        let pos = SectionPos::new(-2097152, 524287, 2097151);
        let mut bytes = Vec::new();
        pos.encode(&mut bytes).unwrap();
        assert_eq!(SectionPos::decode(&mut bytes.as_slice()).unwrap(), pos);
        assert!(
            SectionPos::new(2097152, 0, 0)
                .encode(&mut Vec::new())
                .is_err()
        );
    }
}
