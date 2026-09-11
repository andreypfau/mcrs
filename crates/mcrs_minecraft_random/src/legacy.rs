use crate::{GaussianBank, Random, block_pos_seed};
use bevy_math::IVec3;
use rand_xoshiro::rand_core::{Rng, TryRng};
use std::convert::Infallible;

const MODULUS_BITS: usize = 48;
const MODULUS_MASK: u64 = 281474976710655;
const MULTIPLIER: u64 = 25214903917;
const INCREMENT: u64 = 11;
const F32_MULTIPLIER: f32 = 1.0 / (1u64 << 24) as f32;
const F64_MULTIPLIER: f64 = 1.0 / (1u64 << 53) as f64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyRandom {
    pub seed: u64,
    banked_gaussian: GaussianBank,
}

impl LegacyRandom {
    pub fn new(seed: u64) -> Self {
        Self {
            seed: (seed ^ MULTIPLIER) & MODULUS_MASK,
            banked_gaussian: GaussianBank::default(),
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

    /// Java-accurate `nextLong()`: both 32-bit halves are sign-extended before combining.
    /// Java's `nextLong` computes `((long)(int)upper << 32) + (long)(int)lower`, so when
    /// the lower half has its high bit set the result is reduced by 2^32 relative to the
    /// unsigned interpretation used by `try_next_u64`.
    #[inline]
    pub fn next_java_long(&mut self) -> i64 {
        let hi = self.next_bits(32) as i32 as i64;
        let lo = self.next_bits(32) as i32 as i64;
        (hi << 32).wrapping_add(lo)
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

    fn next_java_long(&mut self) -> i64 {
        LegacyRandom::next_java_long(self)
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

    /// `BitRandomSource.DOUBLE_MULTIPLIER` is declared from the float literal
    /// `1.110223E-16F`, which rounds exactly onto `2^-53`, so the modern source and
    /// `java.util.Random` agree bit for bit here.
    fn next_f64(&mut self) -> f64 {
        let hi = self.next_bits(26);
        let lo = self.next_bits(27);
        ((hi << 27) + lo) as f64 * F64_MULTIPLIER
    }

    fn next_gaussian(&mut self) -> f64 {
        let mut bank = self.banked_gaussian;
        let value = bank.next(|| self.next_f64());
        self.banked_gaussian = bank;
        value
    }

    fn fork(&mut self) -> Self {
        LegacyRandom::new(self.next_java_long() as u64)
    }

    fn fork_at<T>(&mut self, pos: T) -> Self
    where
        T: Into<IVec3>,
    {
        LegacyRandom::new(self.next_java_long() as u64 ^ block_pos_seed(pos))
    }

    /// The legacy factory hashes with `String.hashCode`, not MD5 — MD5 is the Xoroshiro path.
    /// The fold is over bytes, which equals Java's UTF-16 fold for the ASCII names we pass.
    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        let hash = seed
            .as_ref()
            .iter()
            .fold(0i32, |h, &c| h.wrapping_mul(31).wrapping_add(c as i32));
        LegacyRandom::new(self.next_java_long() as u64 ^ hash as i64 as u64)
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
    fn next_gaussian_banks_the_second_of_the_polar_pair() {
        let mut random = LegacyRandom::new(123);
        let mut reference = LegacyRandom::new(123);

        let (x, y, radius_squared) = loop {
            let x = 2.0 * reference.next_f64() - 1.0;
            let y = 2.0 * reference.next_f64() - 1.0;
            let radius_squared = x * x + y * y;
            if radius_squared < 1.0 && radius_squared != 0.0 {
                break (x, y, radius_squared);
            }
        };
        let multiplier = (-2.0 * radius_squared.ln() / radius_squared).sqrt();

        assert_eq!(random.next_gaussian(), x * multiplier);
        assert_eq!(random.next_gaussian(), y * multiplier);
        assert_eq!(
            random.seed, reference.seed,
            "two gaussians must consume exactly one pair of double draws"
        );
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
            assert_eq!(random.next_f64(), e);
        }
    }

    #[test]
    fn next_f64_bit_pattern_matches_java_util_random() {
        let mut random = LegacyRandom::new(123);
        for bits in [
            0x3fe7243e39e5e024u64,
            0x3fefb5719a699f85,
            0x3fd035f4492fa262,
            0x3fe37b4aea123079,
            0x3fe9c9aedcfa9ce4,
        ] {
            assert_eq!(random.next_f64().to_bits(), bits);
        }
    }
}
