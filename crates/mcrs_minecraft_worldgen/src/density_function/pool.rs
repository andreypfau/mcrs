use crate::noise::normal_noise::ColumnScratch;
use bevy_math::IVec3;
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};

/// Scratch buffers lent out for the length of one fill and returned when the
/// lease drops.
///
/// A sampler that evaluates its input over a differently shaped volume — a
/// slice pinning an axis, an interpolation dropping to its cell lattice — needs
/// a buffer the caller never sized for it. Lending it keeps that off the heap
/// once a chunk's first fill has warmed the pool.
#[derive(Default)]
pub(crate) struct BufferPool {
    floats: RefCell<Vec<Vec<f32>>>,
    doubles: RefCell<Vec<Vec<f64>>>,
    positions: RefCell<Vec<Vec<IVec3>>>,
    slots: RefCell<Vec<Vec<u32>>>,
    bools: RefCell<Vec<Vec<bool>>>,
    columns: RefCell<Vec<ColumnScratch>>,
}

/// A buffer borrowed from a [`BufferPool`], returned to it on drop.
pub(crate) struct Lease<'a, T> {
    store: &'a RefCell<Vec<Vec<T>>>,
    buf: Vec<T>,
    len: usize,
}

impl<T> Drop for Lease<'_, T> {
    fn drop(&mut self) {
        self.store.borrow_mut().push(std::mem::take(&mut self.buf));
    }
}

impl<T> Deref for Lease<'_, T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        &self.buf[..self.len]
    }
}

impl<T> DerefMut for Lease<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.buf[..self.len]
    }
}

/// Grown to `len` but never cleared: every caller writes a slot before reading
/// it, so zeroing a recycled buffer would be a memset the size of the volume.
fn lease<T: Clone>(store: &RefCell<Vec<Vec<T>>>, len: usize, filler: T) -> Lease<'_, T> {
    let mut buf = store.borrow_mut().pop().unwrap_or_default();
    if buf.len() < len {
        buf.resize(len, filler);
    }
    Lease { store, buf, len }
}

impl BufferPool {
    pub(crate) fn floats(&self, len: usize) -> Lease<'_, f32> {
        lease(&self.floats, len, 0.0)
    }

    pub(crate) fn doubles(&self, len: usize) -> Lease<'_, f64> {
        lease(&self.doubles, len, 0.0)
    }

    pub(crate) fn slots(&self, len: usize) -> Lease<'_, u32> {
        lease(&self.slots, len, 0)
    }

    pub(crate) fn positions(&self, len: usize) -> Lease<'_, IVec3> {
        lease(&self.positions, len, IVec3::ZERO)
    }

    pub(crate) fn bools(&self, len: usize) -> Lease<'_, bool> {
        lease(&self.bools, len, false)
    }

    pub(crate) fn column_scratch(&self) -> ColumnLease<'_> {
        ColumnLease {
            store: &self.columns,
            scratch: self.columns.borrow_mut().pop().unwrap_or_default(),
        }
    }
}

/// The per-column noise scratch, kept whole so the octave buffers inside it
/// survive between fills.
pub(crate) struct ColumnLease<'a> {
    store: &'a RefCell<Vec<ColumnScratch>>,
    scratch: ColumnScratch,
}

impl Drop for ColumnLease<'_> {
    fn drop(&mut self) {
        self.store
            .borrow_mut()
            .push(std::mem::take(&mut self.scratch));
    }
}

impl Deref for ColumnLease<'_> {
    type Target = ColumnScratch;

    #[inline]
    fn deref(&self) -> &ColumnScratch {
        &self.scratch
    }
}

impl DerefMut for ColumnLease<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut ColumnScratch {
        &mut self.scratch
    }
}
