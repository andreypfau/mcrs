use std::io::Write;

use crate::impls::take;
use crate::{Decode, Encode, ErrorKind, Result};

/// An `i64` encoded with variable length.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[repr(transparent)]
pub struct VarLong(pub i64);

impl From<i64> for VarLong {
    fn from(value: i64) -> Self {
        VarLong(value)
    }
}

impl From<VarLong> for i64 {
    fn from(value: VarLong) -> Self {
        value.0
    }
}

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
    fn encode(&self, mut w: impl Write) -> Result<()> {
        let mut val = self.0 as u64;
        while val >= 0x80 {
            w.write_all(&[val as u8 | 0x80])?;
            val >>= 7;
        }
        w.write_all(&[val as u8])?;
        Ok(())
    }
}

impl Decode<'_> for VarLong {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let mut val = 0;
        for i in 0..Self::MAX_SIZE {
            let [byte] = take(r)?;
            val |= (byte as i64 & 0b01111111) << (i * 7);
            if byte & 0b10000000 == 0 {
                return Ok(VarLong(val));
            }
        }
        Err(ErrorKind::VarIntTooLong.into())
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::{RngExt, SeedableRng};

    use super::*;

    fn check_encode_decode(count: usize) {
        let mut rng = StdRng::seed_from_u64(0x7661_726c_6f6e);
        let mut buf = vec![];

        for n in (0..count)
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

    #[test]
    fn encode_decode() {
        check_encode_decode(10_000);
    }

    mod exhaustive {
        use super::*;

        #[test]
        fn encode_decode() {
            check_encode_decode(1_000_000);
        }
    }
}
