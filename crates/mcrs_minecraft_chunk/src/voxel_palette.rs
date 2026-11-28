use crate::PalettedContainer::{Heterogeneous, Homogeneous};
use crate::{AbstractCube, PalettedContainer};
use mcrs_voxel::{LocalPos, SectionPos};
use std::hash::Hash;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct VoxelPalette<V: Hash + Eq + Copy + Send + Sync + 'static, const DIM: usize>(
    pub PalettedContainer<V, DIM>,
);

impl<V: Hash + Eq + Copy + Send + Sync + 'static, const DIM: usize> VoxelPalette<V, DIM> {
    pub const SIZE: usize = DIM;
    pub const VOLUME: usize = DIM * DIM * DIM;

    pub fn homogeneous(value: V) -> Self {
        Self(PalettedContainer::Homogeneous(value))
    }

    pub fn from_cells(cells: &[V]) -> Option<Self> {
        PalettedContainer::from_cells(cells).map(Self)
    }

    pub fn from_cube(cube: Box<AbstractCube<V, DIM>>) -> Self {
        Self(PalettedContainer::from_cube(cube))
    }

    pub fn fill(&mut self, value: V) {
        self.0 = Homogeneous(value);
    }

    pub fn set_cell(&mut self, x: usize, y: usize, z: usize, value: V) -> Option<V> {
        self.0.set(x, y, z, value)
    }

    pub fn get_cell(&self, x: usize, y: usize, z: usize) -> Option<V> {
        self.0.get(x, y, z)
    }

    #[cfg_attr(
        mcrs_lint,
        expect(
            mcrs::too_many_arguments,
            reason = "the lower and upper bound of the box on each axis, then the value to fill it with"
        )
    )]
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

impl<V: Hash + Eq + Copy + Send + Sync + 'static> VoxelPalette<V, { SectionPos::SIZE }> {
    pub fn get(&self, pos: LocalPos) -> V {
        self.0.get_local(pos)
    }

    pub fn set(&mut self, pos: LocalPos, value: V) -> V {
        self.0.set_local(pos, value)
    }
}

#[derive(Debug)]
pub struct SharedVoxelPalette<V: Hash + Eq + Copy + Send + Sync + 'static, const DIM: usize>(
    pub Arc<VoxelPalette<V, DIM>>,
);

impl<V: Hash + Eq + Copy + Send + Sync + 'static, const DIM: usize> Clone
    for SharedVoxelPalette<V, DIM>
{
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<V: Hash + Eq + Copy + Default + Send + Sync + 'static, const DIM: usize> Default
    for SharedVoxelPalette<V, DIM>
{
    fn default() -> Self {
        Self(Arc::new(VoxelPalette::default()))
    }
}

impl<V: Hash + Eq + Copy + Send + Sync + 'static, const DIM: usize> std::ops::Deref
    for SharedVoxelPalette<V, DIM>
{
    type Target = VoxelPalette<V, DIM>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<V: Hash + Eq + Copy + Send + Sync + 'static, const DIM: usize> SharedVoxelPalette<V, DIM> {
    pub fn new(palette: VoxelPalette<V, DIM>) -> Self {
        Self(Arc::new(palette))
    }

    pub fn make_mut(&mut self) -> &mut VoxelPalette<V, DIM> {
        Arc::make_mut(&mut self.0)
    }
}
