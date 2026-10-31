use mcrs_voxel::BlockStateId;
use mcrs_voxel::{BlockPos, LocalPos, SectionPos};

use crate::access::{Denied, Unavailable, Volume};
use crate::block::{BlockVolume, BlockVolumeMut, SectionBlocks, SectionBlocksMut};
use crate::visit::SectionVolume;

/// A store that keeps block states per position rather than per section: an overlay, a
/// map, a box that is not section-aligned. Implementing it makes the store a
/// [`BlockVolume`] whose section handles read through it.
///
/// `voxel` is asked only for positions inside a section the store grants, and answers for
/// every cell of that section, including those outside the bounds.
pub trait VoxelStore: Volume {
    fn voxel(&self, at: BlockPos) -> BlockStateId;
}

pub trait VoxelStoreMut: VoxelStore {
    /// Answers the state the write replaced. A write to a cell outside the bounds is
    /// dropped.
    fn set_voxel(&mut self, at: BlockPos, state: BlockStateId) -> BlockStateId;
}

pub struct VoxelSection<'a, S: ?Sized> {
    store: &'a S,
    origin: BlockPos,
}

pub struct VoxelSectionMut<'a, S: ?Sized> {
    store: &'a mut S,
    origin: BlockPos,
}

fn origin(section: SectionPos) -> BlockPos {
    let shift = SectionPos::BITS as u32;
    BlockPos::new(
        section.x.wrapping_shl(shift),
        section.y.wrapping_shl(shift),
        section.z.wrapping_shl(shift),
    )
}

fn offset(origin: BlockPos, at: LocalPos) -> BlockPos {
    BlockPos::new(
        origin.x.wrapping_add(i32::from(at.x())),
        origin.y.wrapping_add(i32::from(at.y())),
        origin.z.wrapping_add(i32::from(at.z())),
    )
}

// chisle: `uniform`, `palette`, `for_each` and `copy_to` on these handles make 4096 `voxel`
// calls; a store with dense or paletted sections lifts it by handing out its own handle.
impl<S: VoxelStore + ?Sized> SectionBlocks for VoxelSection<'_, S> {
    #[inline]
    fn get(&self, at: LocalPos) -> BlockStateId {
        self.store.voxel(offset(self.origin, at))
    }
}

impl<S: VoxelStore + ?Sized> SectionBlocks for VoxelSectionMut<'_, S> {
    #[inline]
    fn get(&self, at: LocalPos) -> BlockStateId {
        VoxelSection {
            store: &*self.store,
            origin: self.origin,
        }
        .get(at)
    }
}

impl<S: VoxelStoreMut + ?Sized> SectionBlocksMut for VoxelSectionMut<'_, S> {
    #[inline]
    fn set(&mut self, at: LocalPos, state: BlockStateId) -> BlockStateId {
        self.store.set_voxel(offset(self.origin, at), state)
    }
}

impl<S: VoxelStore> BlockVolume for S {
    type Section<'a>
        = VoxelSection<'a, S>
    where
        S: 'a;

    fn section(&self, at: SectionPos) -> Result<VoxelSection<'_, S>, Unavailable> {
        self.access(at).check_read()?;
        Ok(VoxelSection {
            store: self,
            origin: origin(at),
        })
    }
}

impl<S: VoxelStoreMut> BlockVolumeMut for S {
    type SectionMut<'a>
        = VoxelSectionMut<'a, S>
    where
        S: 'a;

    fn section_mut(&mut self, at: SectionPos) -> Result<VoxelSectionMut<'_, S>, Denied> {
        self.access(at).check_write()?;
        Ok(VoxelSectionMut {
            origin: origin(at),
            store: self,
        })
    }
}

impl<S: VoxelStore> SectionVolume for S {}
