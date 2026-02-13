use bevy_math::IVec3;
use md5::{Digest, Md5};
use rand_xoshiro::Xoroshiro128PlusPlus;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};

use crate::{Random, block_pos_seed};

const F32_MULTIPLIER: f32 = 1.0 / (1u64 << 24) as f32;
const F64_MULTIPLIER: f64 = 1.0 / (1u64 << 53) as f64;
const STAFFORD_1: u64 = 0xbf58476d1ce4e5b9;
const STAFFORD_2: u64 = 0x94d049bb133111eb;
const SILVER_RATIO: u64 = 0x6a09e667f3bcc909;
const GOLDEN_RATIO: u64 = 0x9e3779b97f4a7c15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XoroshiroRandom(Xoroshiro128PlusPlus);

impl XoroshiroRandom {
    pub fn new(seed: u64) -> Self {
        let (lo, hi) = upgrade_seed_to_u128(seed);
        Self::from_u128_seed(lo, hi)
    }

    pub fn from_u128_seed(lo: u64, hi: u64) -> Self {
        let mut array = [0u8; 16];
        array[..8].copy_from_slice(&lo.to_le_bytes());
        array[8..16].copy_from_slice(&hi.to_le_bytes());
        Self(Xoroshiro128PlusPlus::from_seed(array))
    }

    fn next_bits(&mut self, bits: usize) -> u64 {
        self.next_u64() >> (64 - bits)
    }
}

impl Random for XoroshiroRandom {
    fn is_legacy(&self) -> bool {
        false
    }

    fn next_bool(&mut self) -> bool {
        self.next_u64() & 1 != 0
    }

    fn next_u32_bound(&mut self, bound: u32) -> u32 {
        let mut l = self.next_u32() as u64;
        let mut m = l.wrapping_mul(bound as u64);
        let mut n = (m & 0xFFFFFFFF) as u32;
        if n < bound {
            let threshold = (!bound + 1).wrapping_rem(bound);
            while n < threshold {
                l = self.next_u32() as u64;
                m = l.wrapping_mul(bound as u64);
                n = (m & 0xFFFFFFFF) as u32;
            }
        }
        (m >> 32) as u32
    }

    fn next_f32(&mut self) -> f32 {
        self.next_bits(24) as f32 * F32_MULTIPLIER
    }

    fn next_f64(&mut self) -> f64 {
        self.next_bits(53) as f64 * F64_MULTIPLIER
    }

    fn fork(&mut self) -> XoroshiroRandom {
        XoroshiroRandom::from_u128_seed(self.next_u64(), self.next_u64())
    }

    fn fork_at<T>(&mut self, pos: T) -> Self
    where
        T: Into<IVec3>,
    {
        let seed = block_pos_seed(pos);
        let lo = self.next_u64() ^ seed;
        let hi = self.next_u64();
        XoroshiroRandom::from_u128_seed(lo, hi)
    }

    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        let l = self.next_u64();
        let h = self.next_u64();
        let mut hasher = Md5::new();
        hasher.update(seed);
        let hash = hasher.finalize();
        let lo = u64::from_be_bytes(hash[0..8].try_into().unwrap());
        let hi = u64::from_be_bytes(hash[8..16].try_into().unwrap());
        XoroshiroRandom::from_u128_seed(lo ^ l, hi ^ h)
    }
}

impl RngCore for XoroshiroRandom {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }

    #[inline]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest)
    }
}

fn mix_starford_13(mut v: u64) -> u64 {
    v = (v ^ v >> 30).wrapping_mul(STAFFORD_1);
    v = (v ^ v >> 27).wrapping_mul(STAFFORD_2);
    v ^ v >> 31
}

fn upgrade_seed_to_u128(seed: u64) -> (u64, u64) {
    let lo = seed ^ SILVER_RATIO;
    let hi = lo.wrapping_add(GOLDEN_RATIO);
    (mix_starford_13(lo), mix_starford_13(hi))
}

#[cfg(test)]
mod test {
    use crate::Random;
    use crate::xoroshiro::XoroshiroRandom;

    #[test]
    fn next_i64() {
        let mut random = XoroshiroRandom::new(1);
        let expected = [
            -1033667707219518978,
            6451672561743293322,
            -1821890263888393630,
            890086654470169703,
            8094835630745194324,
            2779418831538184155,
            -2153570570747265786,
            2631759950516672506,
            1341645417244425603,
            -2886123833362855573,
        ];
        for &e in &expected {
            assert_eq!(random.next_i64(), e);
        }
    }

    #[test]
    fn next_i32() {
        let mut random = XoroshiroRandom::new(1);
        let expected = [
            1734564350,
            836234122,
            825264738,
            -1425890201,
            767430484,
            -2015535141,
            -606094074,
            950360058,
            224558467,
            916343147,
        ];
        for &e in &expected {
            assert_eq!(random.next_i32(), e);
        }
    }

    #[test]
    fn next_u32_bound() {
        let mut random = XoroshiroRandom::new(1);
        assert_eq!(random.next_u32_bound(25), 10);
        assert_eq!(random.next_u32_bound(256), 49);
        assert_eq!(random.next_u32_bound(255), 48);
        assert_eq!(random.next_u32_bound(254), 169);
        assert_eq!(random.next_u32_bound(0x7FFFFFFF), 383715241);
    }

    #[test]
    fn next_f32() {
        let mut random = XoroshiroRandom::new(1);
        let expected = [
            0.9439647, 0.34974587, 0.9012351, 0.04825169, 0.4388219, 0.15067255, 0.88325465,
            0.14266795, 0.07273072, 0.8435429,
        ];
        for &e in &expected {
            assert_eq!(random.next_f32(), e);
        }
    }

    #[test]
    fn next_f64() {
        let mut random = XoroshiroRandom::new(1);
        let expected = [
            0.9439647613102243,
            0.34974587038035987,
            0.9012351308931007,
            0.048251694223845565,
            0.4388219188383503,
            0.15067259677004097,
            0.8832547054297483,
            0.1426679927905259,
            0.07273074380408129,
            0.84354291349029,
        ];
        for &e in &expected {
            assert_eq!(random.next_f64(), e);
        }
    }
}
