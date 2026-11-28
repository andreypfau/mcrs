use std::cell::Cell;
use std::rc::Rc;

use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use rand_xoshiro::rand_core::{Infallible, TryRng};

#[derive(Clone)]
pub(crate) struct CountingRng {
    inner: LegacyRandom,
    draws: Rc<Cell<u64>>,
}

impl CountingRng {
    pub(crate) fn new(seed: u64, draws: Rc<Cell<u64>>) -> Self {
        CountingRng {
            inner: LegacyRandom::new(seed),
            draws,
        }
    }
    fn inc(&self) {
        self.draws.set(self.draws.get() + 1);
    }
}

impl TryRng for CountingRng {
    type Error = Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        self.inc();
        self.inner.try_next_u32()
    }
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        self.inc();
        self.inner.try_next_u64()
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        for _ in 0..dst.len().div_ceil(8) {
            self.inc();
        }
        self.inner.try_fill_bytes(dst)
    }
}

impl Random for CountingRng {
    fn is_legacy(&self) -> bool {
        true
    }
    fn next_bool(&mut self) -> bool {
        self.inc();
        self.inner.next_bool()
    }
    fn next_u32_bound(&mut self, bound: u32) -> u32 {
        self.inc();
        self.inner.next_u32_bound(bound)
    }
    fn next_f32(&mut self) -> f32 {
        self.inc();
        self.inner.next_f32()
    }
    fn next_f64(&mut self) -> f64 {
        self.inc();
        self.inc();
        self.inner.next_f64()
    }

    fn next_gaussian(&mut self) -> f64 {
        self.inner.next_gaussian()
    }
    fn fork(&mut self) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork(),
            draws: self.draws.clone(),
        }
    }
    fn fork_at<T: Into<glam::IVec3>>(&mut self, pos: T) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork_at(pos),
            draws: self.draws.clone(),
        }
    }
    fn fork_hash(&mut self, seed: impl AsRef<[u8]>) -> Self {
        self.inc();
        CountingRng {
            inner: self.inner.fork_hash(seed),
            draws: self.draws.clone(),
        }
    }
}
