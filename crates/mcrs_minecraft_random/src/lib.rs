pub mod legacy;
pub mod xoroshiro;

use crate::legacy::LegacyRandom;
use crate::xoroshiro::XoroshiroRandom;
use bevy_math::IVec3;
use rand_xoshiro::rand_core::{Rng, TryRng};
use std::convert::Infallible;

pub trait Random: Rng + Clone {
    fn is_legacy(&self) -> bool;

    fn next_bool(&mut self) -> bool;

    fn next_i32(&mut self) -> i32 {
        self.next_u32() as i32
    }

    fn next_u32_bound(&mut self, bound: u32) -> u32;

    fn next_i32_bound(&mut self, bound: i32) -> i32 {
        self.next_u32_bound(bound as u32) as i32
    }

    fn next_i64(&mut self) -> i64 {
        self.next_u64() as i64
    }

    /// Java-accurate `nextLong()`: sign-extends both 32-bit halves before combining.
    ///
    /// Java's `java.util.Random.nextLong` computes `((long)(int)upper << 32) + (long)(int)lower`,
    /// so when the lower half has its high bit set the result is less than the unsigned
    /// interpretation. The default delegates to `next_i64()` (correct for Xoroshiro);
    /// LegacyRandom overrides with the sign-extended form.
    fn next_java_long(&mut self) -> i64 {
        self.next_i64()
    }

    fn next_f32(&mut self) -> f32;

    fn next_f64(&mut self) -> f64;

    fn fork(&mut self) -> Self;

    fn fork_at<T>(&mut self, pos: T) -> Self
    where
        T: Into<IVec3>;

    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self;
}

fn block_pos_seed<T>(pos: T) -> u64
where
    T: Into<IVec3>,
{
    let pos = pos.into();
    // The asymmetry is the data: Java's `x * 3129871` is an int multiply that wraps at 32
    // bits, while `z * 116129781L` is a long multiply that does not.
    let mut l = (pos.x.wrapping_mul(3129871) as i64)
        ^ (pos.z as i64).wrapping_mul(116129781)
        ^ (pos.y as i64);
    l = l
        .wrapping_mul(l)
        .wrapping_mul(42317861)
        .wrapping_add(l.wrapping_mul(11));
    (l >> 16) as u64
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RandomSource {
    Legacy(LegacyRandom),
    Xoroshiro(XoroshiroRandom),
}

impl RandomSource {
    pub fn new(seed: u64, legacy: bool) -> Self {
        if legacy {
            RandomSource::Legacy(LegacyRandom::new(seed))
        } else {
            RandomSource::Xoroshiro(XoroshiroRandom::new(seed))
        }
    }
}

impl TryRng for RandomSource {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(match self {
            RandomSource::Legacy(random) => random.next_u32(),
            RandomSource::Xoroshiro(random) => random.next_u32(),
        })
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(match self {
            RandomSource::Legacy(random) => random.next_u64(),
            RandomSource::Xoroshiro(random) => random.next_u64(),
        })
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        match self {
            RandomSource::Legacy(random) => random.fill_bytes(dest),
            RandomSource::Xoroshiro(random) => random.fill_bytes(dest),
        }
        Ok(())
    }
}

impl Random for RandomSource {
    fn is_legacy(&self) -> bool {
        match self {
            RandomSource::Legacy(_) => true,
            RandomSource::Xoroshiro(_) => false,
        }
    }

    fn next_bool(&mut self) -> bool {
        match self {
            RandomSource::Legacy(random) => random.next_bool(),
            RandomSource::Xoroshiro(random) => random.next_bool(),
        }
    }

    fn next_u32_bound(&mut self, bound: u32) -> u32 {
        match self {
            RandomSource::Legacy(random) => random.next_u32_bound(bound),
            RandomSource::Xoroshiro(random) => random.next_u32_bound(bound),
        }
    }

    fn next_java_long(&mut self) -> i64 {
        match self {
            RandomSource::Legacy(random) => random.next_java_long(),
            RandomSource::Xoroshiro(random) => random.next_java_long(),
        }
    }

    fn next_f32(&mut self) -> f32 {
        match self {
            RandomSource::Legacy(random) => random.next_f32(),
            RandomSource::Xoroshiro(random) => random.next_f32(),
        }
    }

    fn next_f64(&mut self) -> f64 {
        match self {
            RandomSource::Legacy(random) => random.next_f64(),
            RandomSource::Xoroshiro(random) => random.next_f64(),
        }
    }

    fn fork(&mut self) -> Self {
        match self {
            RandomSource::Legacy(random) => RandomSource::Legacy(random.fork()),
            RandomSource::Xoroshiro(random) => RandomSource::Xoroshiro(random.fork()),
        }
    }

    fn fork_at<T>(&mut self, pos: T) -> Self
    where
        T: Into<IVec3>,
    {
        match self {
            RandomSource::Legacy(random) => RandomSource::Legacy(random.fork_at(pos)),
            RandomSource::Xoroshiro(random) => RandomSource::Xoroshiro(random.fork_at(pos)),
        }
    }

    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        match self {
            RandomSource::Legacy(random) => RandomSource::Legacy(random.fork_hash(seed)),
            RandomSource::Xoroshiro(random) => RandomSource::Xoroshiro(random.fork_hash(seed)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn block_pos_seed_matches_java() {
        for (pos, expected) in [
            ([100, 64, 100], -52100398098179i64),
            ([5, 64, 19], -38667155502638),
            ([-2048, -64, 3000], -67643415488191),
            ([0, 0, 0], 0),
            ([-1, -1, -1], 60311958933234),
        ] {
            let pos = IVec3::new(pos[0], pos[1], pos[2]);
            assert_eq!(block_pos_seed(pos) as i64, expected, "{pos:?}");
        }
    }

    #[test]
    fn a_legacy_positional_factory_matches_java() {
        let mut root = LegacyRandom::new(42);
        assert_eq!(
            root.clone().fork_hash("minecraft:bedrock_roof"),
            LegacyRandom::new(-5025562857781560243i64 as u64)
        );
        assert_eq!(
            root.clone().fork_hash("minecraft:bedrock_floor"),
            LegacyRandom::new(-5025562856259330031i64 as u64)
        );
        assert_eq!(
            root.clone().fork_hash("octave_-3"),
            LegacyRandom::new(-5025562857811928990i64 as u64)
        );
        assert_eq!(
            root.fork_at(IVec3::new(100, 64, 100)),
            LegacyRandom::new(5025539619815019018i64 as u64)
        );
    }
}
