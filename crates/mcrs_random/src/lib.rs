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
    let mut l = (pos.x.wrapping_mul(3129871) as i64)
        ^ (pos.z.wrapping_mul(116129781) as i64)
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
