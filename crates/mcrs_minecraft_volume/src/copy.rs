use std::ops::ControlFlow;

use mcrs_minecraft_core::{
    BlockPos, BlockStateId, BoundingBox, LocalPos, Mirror, Rotation, SectionPos,
};

use crate::access::{Denied, Unavailable, within};
use crate::block::{BlockVolumeMut, SectionBlocks};
use crate::bounds::SectionBox;
use crate::visit::{Missing, SectionVolume, VisitOptions, VisitOrder};

/// Where a cell of one box lands in another: mirrored, rotated about `pivot` on the
/// horizontal plane, then moved by `offset`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Transform {
    pub mirror: Mirror,
    pub rotation: Rotation,
    pub pivot: BlockPos,
    pub offset: BlockPos,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        mirror: Mirror::None,
        rotation: Rotation::None,
        pivot: BlockPos::new(0, 0, 0),
        offset: BlockPos::new(0, 0, 0),
    };

    pub fn apply(&self, at: BlockPos) -> BlockPos {
        let (x, y, z) = match self.mirror {
            Mirror::None => (at.x, at.y, at.z),
            Mirror::LeftRight => (at.x, at.y, at.z.wrapping_neg()),
            Mirror::FrontBack => (at.x.wrapping_neg(), at.y, at.z),
        };
        let (px, pz) = (self.pivot.x, self.pivot.z);
        let (x, z) = match self.rotation {
            Rotation::None => (x, z),
            Rotation::Counterclockwise90 => (
                px.wrapping_sub(pz).wrapping_add(z),
                px.wrapping_add(pz).wrapping_sub(x),
            ),
            Rotation::Clockwise90 => (
                px.wrapping_add(pz).wrapping_sub(z),
                pz.wrapping_sub(px).wrapping_add(x),
            ),
            Rotation::Clockwise180 => (
                px.wrapping_add(px).wrapping_sub(x),
                pz.wrapping_add(pz).wrapping_sub(z),
            ),
        };
        BlockPos::new(
            x.wrapping_add(self.offset.x),
            y.wrapping_add(self.offset.y),
            z.wrapping_add(self.offset.z),
        )
    }

    /// The box the cells of `area` land in.
    pub fn apply_box(&self, area: BoundingBox) -> BoundingBox {
        BoundingBox::from_corners(self.apply(area.min), self.apply(area.max))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum CopyError {
    #[error("a source section cannot be read: {0}")]
    Source(Unavailable),
    #[error("a target section cannot be written: {0}")]
    Target(Denied),
}

/// Copies every block of `area` in `from` to where `transform` places it in `to`, each
/// state through `state`, which is where a caller rotates and mirrors the states
/// themselves. All or nothing: no block moves unless every source section is readable
/// and every target section writable. Answers the number of blocks written.
pub fn copy_blocks<S, T, F>(
    from: &S,
    area: BoundingBox,
    to: &mut T,
    transform: Transform,
    state: F,
) -> Result<u64, CopyError>
where
    S: SectionVolume + ?Sized,
    T: BlockVolumeMut + ?Sized,
    F: Fn(BlockStateId) -> BlockStateId,
{
    let sources = SectionBox::covering(area);
    for at in sources.iter(VisitOrder::SectionMajor) {
        from.access(at).check_read().map_err(CopyError::Source)?;
    }
    let targets = SectionBox::covering(transform.apply_box(area));
    for at in targets.iter(VisitOrder::SectionMajor) {
        to.access(at).check_write().map_err(CopyError::Target)?;
    }

    let options = VisitOptions {
        order: VisitOrder::SectionMajor,
        missing: Missing::Report,
    };
    let mut written = 0u64;
    let flow = from.visit_sections(sources, options, |at, section| {
        let section = match section {
            Ok(section) => section,
            Err(why) => return ControlFlow::Break(CopyError::Source(why)),
        };
        let origin = BlockPos::new(
            at.x.wrapping_mul(SectionPos::SIZE as i32),
            at.y.wrapping_mul(SectionPos::SIZE as i32),
            at.z.wrapping_mul(SectionPos::SIZE as i32),
        );
        let mut failed = None;
        section.for_each(|local, cell| {
            if failed.is_some() {
                return;
            }
            let pos = offset(origin, local);
            if !within(area, pos) {
                return;
            }
            match to.set_block(transform.apply(pos), state(cell)) {
                Ok(_) => written = written.saturating_add(1),
                Err(why) => failed = Some(CopyError::Target(why)),
            }
        });
        match failed {
            Some(why) => ControlFlow::Break(why),
            None => ControlFlow::Continue(()),
        }
    });
    match flow {
        ControlFlow::Break(why) => Err(why),
        ControlFlow::Continue(()) => Ok(written),
    }
}

fn offset(origin: BlockPos, local: LocalPos) -> BlockPos {
    BlockPos::new(
        origin.x.wrapping_add(i32::from(local.x())),
        origin.y.wrapping_add(i32::from(local.y())),
        origin.z.wrapping_add(i32::from(local.z())),
    )
}
