use std::ops::ControlFlow;

use mcrs_minecraft_core::SectionPos;

use crate::access::Unavailable;
use crate::block::BlockVolume;
use crate::bounds::SectionBox;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VisitOrder {
    /// `y`, then `z`, then `x`, `x` fastest.
    SectionMajor,
    /// Column by column: `x` slowest, then `z`, then `y` within a column.
    ColumnMajor,
    /// Whatever order the store walks cheapest.
    Any,
}

/// What a visit does with a section it cannot read.
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
    /// Hands `f` every section of `area` in `options.order` until it breaks. The default
    /// walks the whole area; a store that holds few of its sections walks only those.
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
