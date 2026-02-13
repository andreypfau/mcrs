use crate::{Random, block_pos_seed};
use bevy_math::IVec3;
use md5::{Digest, Md5};
use rand_xoshiro::rand_core::{Rng, TryRng};
use std::convert::Infallible;

const MODULUS_BITS: usize = 48;
const MODULUS_MASK: u64 = 281474976710655;
const MULTIPLIER: u64 = 25214903917;
const INCREMENT: u64 = 11;
const F32_MULTIPLIER: f32 = 1.0 / (1u64 << 24) as f32;
const F64_MULTIPLIER: f64 = 1.0 / (1u64 << 30) as f64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRandom {
    pub seed: u64,
}

impl LegacyRandom {
    pub fn new(seed: u64) -> Self {
        Self {
            seed: (seed ^ MULTIPLIER) & MODULUS_MASK,
        }
    }

    #[inline]
    fn advance(&mut self) {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT) & MODULUS_MASK;
    }

    fn next_bits(&mut self, bits: usize) -> u64 {
        self.advance();
        self.seed >> (MODULUS_BITS - bits)
    }
}

impl TryRng for LegacyRandom {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.next_bits(32) as u32)
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok((self.next_bits(32) << 32) + self.next_bits(32))
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        // Implement fill_bytes using next_u64
        let mut i = 0;
        while i + 8 <= dest.len() {
            let bytes = self.next_u64().to_le_bytes();
            dest[i..i + 8].copy_from_slice(&bytes);
            i += 8;
        }
        if i < dest.len() {
            let bytes = self.next_u64().to_le_bytes();
            let remaining = dest.len() - i;
            dest[i..].copy_from_slice(&bytes[..remaining]);
        }
        Ok(())
    }
}


impl Random for LegacyRandom {
    fn is_legacy(&self) -> bool {
        true
    }

    fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    fn next_u32_bound(&mut self, bound: u32) -> u32 {
        if (bound & (bound - 1)) == 0 {
            let n = self.next_bits(31);
            return ((bound as u64).wrapping_mul(n) >> 31) as u32;
        }
        let mut a;
        let mut b;
        loop {
            a = self.next_bits(31) as i64;
            b = a % bound as i64;
            if a - b + (bound as i64 - 1) >= 0 {
                break;
            }
        }
        b as u32
    }

    fn next_f32(&mut self) -> f32 {
        self.next_bits(24) as f32 * F32_MULTIPLIER
    }

    fn next_f64(&mut self) -> f64 {
        let res = self.next_bits(30) as f64 * F64_MULTIPLIER;
        self.advance();
        res
    }

    fn fork(&mut self) -> Self {
        LegacyRandom::new(self.next_u64())
    }

    fn fork_at<T>(&mut self, pos: T) -> Self
    where
        T: Into<IVec3>,
    {
        LegacyRandom::new(self.next_u64() ^ block_pos_seed(pos))
    }

    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        let mut hasher = Md5::new();
        hasher.update(seed);
        let hash = hasher.finalize();
        LegacyRandom::new(self.next_u64() ^ u64::from_le_bytes(hash[0..8].try_into().unwrap()))
    }
}

#[cfg(test)]
mod test {
    use crate::Random;
    use crate::legacy::LegacyRandom;

    #[test]
    fn next_i32() {
        let mut random = LegacyRandom::new(123);
        let expected = [
            -1188957731,
            1018954901,
            -39088943,
            1295249578,
            1087885590,
            -1829099982,
            -1680189627,
            1111887674,
            -833784125,
            -1621910390,
        ];
        for e in expected {
            assert_eq!(random.next_i32(), e);
        }
    }

    #[test]
    fn next_i32_bound() {
        let mut random = LegacyRandom::new(123);
        assert_eq!(random.next_i32_bound(256), 185);
        assert_eq!(random.next_i32_bound(255), 200);
        assert_eq!(random.next_i32_bound(254), 74);
    }

    #[test]
    fn next_f32() {
        let mut random = LegacyRandom::new(123);
        let expected = [
            0.72317415, 0.23724389, 0.99089885, 0.30157375, 0.2532931, 0.57412946, 0.60880035,
            0.2588815, 0.80586946, 0.6223695,
        ];
        for e in expected {
            assert_eq!(random.next_f32(), e);
        }
    }

    #[test]
    fn next_f64() {
        let mut random = LegacyRandom::new(123);
        let expected = [
            0.7231742029971469,
            0.9908988967772393,
            0.25329310557439133,
            0.6088003703785169,
            0.8058695140834087,
            0.8754127852514174,
            0.7160485112997248,
            0.07191702249367171,
            0.7962609718390335,
            0.5787169373422367,
        ];
        for e in expected {
            assert_eq!(format!("{:.7}", random.next_f64()), format!("{:.7}", e));
        }
    }
}
