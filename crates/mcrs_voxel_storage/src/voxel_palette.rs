use crate::PalettedContainer;
use crate::PalettedContainer::{Heterogeneous, Homogeneous};
use bevy_ecs::component::Component;
use mcrs_voxel_math::BlockPos;
use std::hash::Hash;

/// One section's cube of voxel ids, addressed by section-local position.
#[derive(Component, Debug, Clone, Default)]
pub struct VoxelPalette<V: Hash + Eq + Copy + Default + Send + Sync + 'static, const DIM: usize>(
    pub PalettedContainer<V, DIM>,
);

impl<V: Hash + Eq + Copy + Default + Send + Sync + 'static, const DIM: usize> VoxelPalette<V, DIM> {
    pub const SIZE: usize = DIM;
    const MASK: usize = DIM - 1;

    pub fn fill(&mut self, value: V) {
        self.0 = Homogeneous(value);
    }

    pub fn get<I: Into<BlockPos>>(&self, pos: I) -> V {
        let pos = pos.into();
        self.0.get(
            pos.x as usize & Self::MASK,
            pos.y as usize & Self::MASK,
            pos.z as usize & Self::MASK,
        )
    }

    pub fn set<I: Into<BlockPos>>(&mut self, pos: I, value: V) -> V {
        let pos = pos.into();
        self.0.set(
            pos.x as usize & Self::MASK,
            pos.y as usize & Self::MASK,
            pos.z as usize & Self::MASK,
            value,
        )
    }

    pub fn set_cell(&mut self, x: usize, y: usize, z: usize, value: V) -> V {
        self.0.set(x, y, z, value)
    }

    /// Fill the box `[x0, x1) x [y0, y1) x [z0, z1)` in section-local coords.
    /// Produces output identical to per-voxel `set` calls over the same box,
    /// with bulk-optimized palette bookkeeping.
    pub fn fill_box(
        &mut self,
        x0: usize,
        x1: usize,
        y0: usize,
        y1: usize,
        z0: usize,
        z1: usize,
        value: V,
    ) {
        self.0.fill_box(x0, x1, y0, y1, z0, z1, value);
    }

    /// Invoke `f` once for each distinct value present in the container,
    /// without duplicates.
    pub fn for_each_distinct<F: FnMut(V)>(&self, mut f: F) {
        match &self.0 {
            Homogeneous(value) => f(*value),
            Heterogeneous(data) => {
                for value in data.palette.iter() {
                    f(*value);
                }
            }
        }
    }
}
