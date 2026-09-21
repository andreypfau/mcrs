use bevy_math::IVec3;
use rand_xoshiro::rand_core::{Rng, TryRng};
use std::convert::Infallible;

use crate::bits::BitSource;
use crate::xoroshiro::XoroshiroRandom;
use crate::{GaussianBank, Random};

/// `WorldgenRandom` over a Xoroshiro source: the stream every placed feature
/// and structure piece draws from during decoration. It is a
/// `LegacyRandomSource` whose `next(bits)` is the top bits of one Xoroshiro
/// long, so `nextInt(bound)`, `nextBoolean`, `nextDouble` and `nextLong` all
/// differ from the same source read natively; only `nextFloat` agrees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldgenRandom {
    source: XoroshiroRandom,
    banked_gaussian: GaussianBank,
}

impl WorldgenRandom {
    pub fn new(seed: u64) -> Self {
        Self::over(XoroshiroRandom::new(seed))
    }

    pub fn over(source: XoroshiroRandom) -> Self {
        Self {
            source,
            banked_gaussian: GaussianBank::default(),
        }
    }

    /// The wrapped source, which `fork` and `forkPositional` hand out natively.
    pub fn source(&self) -> &XoroshiroRandom {
        &self.source
    }
}

impl BitSource for WorldgenRandom {
    fn next_bits(&mut self, bits: usize) -> u64 {
        self.source.next_u64() >> (64 - bits)
    }

    fn gaussian_bank(&mut self) -> &mut GaussianBank {
        &mut self.banked_gaussian
    }
}

impl TryRng for WorldgenRandom {
    type Error = Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.next_bits(32) as u32)
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.bits_u64())
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.bits_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

impl Random for WorldgenRandom {
    fn is_legacy(&self) -> bool {
        false
    }

    fn next_java_long(&mut self) -> i64 {
        self.bits_java_long()
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
        Self::over(self.source.fork())
    }

    fn fork_at<T>(&mut self, pos: T) -> Self
    where
        T: Into<IVec3>,
    {
        Self::over(self.source.fork_at(pos))
    }

    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        Self::over(self.source.fork_hash(seed))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// `nextInt(16)` is the top four bits of the long, not the four the native
    /// source multiplies out of its low word.
    #[test]
    fn a_bounded_draw_takes_the_top_bits() {
        let mut worldgen = WorldgenRandom::new(2);
        let mut raw = XoroshiroRandom::new(2);
        let long = raw.next_u64();
        assert_eq!(worldgen.next_i32_bound(16) as u64, long >> 60);
    }

    #[test]
    fn a_long_spends_two_draws() {
        let mut worldgen = WorldgenRandom::new(7);
        let mut raw = XoroshiroRandom::new(7);
        let hi = (raw.next_u64() >> 32) as i32 as i64;
        let lo = (raw.next_u64() >> 32) as i32 as i64;
        assert_eq!(worldgen.next_java_long(), (hi << 32).wrapping_add(lo));
    }
}
