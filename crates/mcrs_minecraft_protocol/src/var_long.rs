use std::io::Write;

use anyhow::bail;
use byteorder::{ReadBytesExt, WriteBytesExt};
use derive_more::{From, Into};
use serde::{Deserialize, Serialize};

use crate::{Decode, Encode};

/// An `i64` encoded with variable length.
#[derive(
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Debug,
    From,
    Into,
    Serialize,
    Deserialize,
)]
#[serde(transparent)]
#[repr(transparent)]
pub struct VarLong(pub i64);

impl VarLong {
    /// The maximum number of bytes a `VarLong` can occupy when read from and
    /// written to the Minecraft protocol.
    pub const MAX_SIZE: usize = 10;

    /// Returns the exact number of bytes this varlong will write when
    /// [`Encode::encode`] is called, assuming no error occurs.
    pub fn written_size(self) -> usize {
        match self.0 {
            0 => 1,
            n => (63 - n.leading_zeros() as usize) / 7 + 1,
        }
    }
}

impl Encode for VarLong {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let mut val = self.0 as u64;
        while val >= 0x80 {
            w.write_u8(val as u8 | 0x80)?;
            val >>= 7;
        }
        w.write_u8(val as u8)?;
        Ok(())
    }
}

impl Decode<'_> for VarLong {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let mut val = 0;
        for i in 0..Self::MAX_SIZE {
            let byte = r.read_u8()?;
            val |= (byte as i64 & 0b01111111) << (i * 7);
            if byte & 0b10000000 == 0 {
                return Ok(VarLong(val));
            }
        }
        bail!("VarInt is too large")
    }
}

#[cfg(test)]
mod tests {
    use rand::RngExt;

    use super::*;

    #[test]
    fn encode_decode() {
        let mut rng = rand::rng();
        let mut buf = vec![];

        for n in (0..1_000_000)
            .map(|_| rng.random())
            .chain([0, i64::MIN, i64::MAX])
        {
            VarLong(n).encode(&mut buf).unwrap();

            let mut slice = buf.as_slice();
            assert!(slice.len() <= VarLong::MAX_SIZE);

            assert_eq!(n, VarLong::decode(&mut slice).unwrap().0);
            assert!(slice.is_empty());
            buf.clear();
        }
    }
}
