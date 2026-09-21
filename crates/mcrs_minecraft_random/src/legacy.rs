use crate::bits::BitSource;
use crate::{GaussianBank, Random, block_pos_seed};
use bevy_math::IVec3;
use rand_xoshiro::rand_core::{Rng, TryRng};
use std::convert::Infallible;

const MODULUS_BITS: usize = 48;
const MODULUS_MASK: u64 = 281474976710655;
const MULTIPLIER: u64 = 25214903917;
const INCREMENT: u64 = 11;

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

    /// `WorldgenRandom.setLargeFeatureSeed`.
    ///
    /// Beta's sibling adds odd-forced products where this one exclusive-ors plain
    /// ones; the two are the same idea and not the same number.
    pub fn large_feature_seed(world_seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
        let mut rng = LegacyRandom::new(world_seed as u64);
        let x_scale = rng.next_java_long();
        let z_scale = rng.next_java_long();
        (chunk_x as i64).wrapping_mul(x_scale) ^ (chunk_z as i64).wrapping_mul(z_scale) ^ world_seed
    }

    pub fn large_feature(world_seed: i64, chunk_x: i32, chunk_z: i32) -> Self {
        LegacyRandom::new(Self::large_feature_seed(world_seed, chunk_x, chunk_z) as u64)
    }

    pub fn large_feature_with_salt(world_seed: i64, x: i32, z: i32, salt: i32) -> Self {
        let seed = (x as i64)
            .wrapping_mul(341873128712)
            .wrapping_add((z as i64).wrapping_mul(132897987541))
            .wrapping_add(world_seed)
            .wrapping_add(salt as i64);
        LegacyRandom::new(seed as u64)
    }

    #[inline]
    pub fn next_java_long(&mut self) -> i64 {
        self.bits_java_long()
    }
}

impl BitSource for LegacyRandom {
    fn next_bits(&mut self, bits: usize) -> u64 {
        self.advance();
        self.seed >> (MODULUS_BITS - bits)
    }

    fn gaussian_bank(&mut self) -> &mut GaussianBank {
        &mut self.banked_gaussian
    }
}

impl TryRng for LegacyRandom {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.next_bits(32) as u32)
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.bits_u64())
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
        self.bits_bool()
    }

    fn next_u32_bound(&mut self, bound: u32) -> u32 {
        self.bits_u32_bound(bound)
    }

    fn next_f32(&mut self) -> f32 {
        self.bits_f32()
    }

    fn next_f64(&mut self) -> f64 {
        self.bits_f64()
    }

    fn next_gaussian(&mut self) -> f64 {
        self.bits_gaussian()
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
    fn large_feature_seed_is_the_reference_formula() {
        for (seed, cx, cz) in [(12345i64, 0i32, 0i32), (-9, 17, -33), (1, -1, 1)] {
            let mut rng = LegacyRandom::new(seed as u64);
            let x_scale = rng.next_java_long();
            let z_scale = rng.next_java_long();
            let expected =
                (cx as i64).wrapping_mul(x_scale) ^ (cz as i64).wrapping_mul(z_scale) ^ seed;
            assert_eq!(LegacyRandom::large_feature_seed(seed, cx, cz), expected);
        }
    }

    #[test]
    fn large_feature_sources_ignore_the_top_sixteen_seed_bits() {
        let seed = -6_723_991_117_364_058_231i64;
        for flipped in [seed ^ (1 << 63), seed ^ (0xFFFF << 48)] {
            assert_eq!(
                LegacyRandom::large_feature_with_salt(seed, -7, 12, 10387312),
                LegacyRandom::large_feature_with_salt(flipped, -7, 12, 10387312)
            );
            assert_eq!(
                LegacyRandom::large_feature(seed, -7, 12),
                LegacyRandom::large_feature(flipped, -7, 12)
            );
        }
        assert_ne!(
            LegacyRandom::large_feature_with_salt(seed, -7, 12, 10387312),
            LegacyRandom::large_feature_with_salt(seed ^ 1, -7, 12, 10387312)
        );
    }

    #[test]
    fn next_i32_bound() {
        let mut random = LegacyRandom::new(123);
        assert_eq!(random.next_i32_bound(256), 185);
        assert_eq!(random.next_i32_bound(255), 200);
        assert_eq!(random.next_i32_bound(254), 74);
    }

    #[test]
    fn next_i32_bound_rejects_the_partial_top_bucket() {
        let mut random = LegacyRandom::new(256);
        let draws: Vec<i32> = (0..4).map(|_| random.next_i32_bound(0x6000_0000)).collect();
        assert_eq!(draws, [1129860750, 1377133019, 321559793, 615784825]);
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
