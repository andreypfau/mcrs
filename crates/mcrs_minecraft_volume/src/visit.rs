use std::ops::ControlFlow;

use mcrs_voxel::SectionPos;

use crate::access::Unavailable;
use crate::block::BlockVolume;
use crate::bounds::SectionBox;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VisitOrder {
    SectionMajor,
    ColumnMajor,
    Any,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Missing {
    Skip,
    Report,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VisitOptions {
    pub order: VisitOrder,
    pub missing: Missing,
}

pub trait SectionVolume: BlockVolume {
    fn visit_sections<B, F>(&self, area: SectionBox, options: VisitOptions, f: F) -> ControlFlow<B>
    where
        F: FnMut(SectionPos, Result<Self::Section<'_>, Unavailable>) -> ControlFlow<B>,
    {
        walk(self, area, options, f)
    }
}

pub(crate) fn walk<V, B, F>(
    volume: &V,
    area: SectionBox,
    options: VisitOptions,
    mut f: F,
) -> ControlFlow<B>
where
    V: BlockVolume + ?Sized,
    F: FnMut(SectionPos, Result<V::Section<'_>, Unavailable>) -> ControlFlow<B>,
{
    let mut visit = |at: SectionPos| match volume.section(at) {
        Err(_) if options.missing == Missing::Skip => ControlFlow::Continue(()),
        section => f(at, section),
    };
    area.iter(options.order).try_for_each(&mut visit)
}
