use crate::visit::VisitOrder;
use mcrs_minecraft_core::BoundingBox;
use mcrs_voxel::SectionPos;

/// An inclusive box of sections; empty when `min` exceeds `max` on any axis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SectionBox {
    pub min: SectionPos,
    pub max: SectionPos,
}

impl SectionBox {
    pub fn covering(blocks: BoundingBox) -> Self {
        Self {
            min: SectionPos::from(blocks.min),
            max: SectionPos::from(blocks.max),
        }
    }

    pub fn contains(&self, at: SectionPos) -> bool {
        (self.min.cmple(at.0) & at.cmple(self.max.0)).all()
    }

    /// Sections in the given order; `Any` walks section-major.
    pub fn iter(&self, order: VisitOrder) -> impl Iterator<Item = SectionPos> + use<> {
        let (min, max) = (self.min, self.max);
        let column_major = order == VisitOrder::ColumnMajor;
        let (outer, inner) = if column_major {
            (min.x..=max.x, min.y..=max.y)
        } else {
            (min.y..=max.y, min.x..=max.x)
        };
        outer.flat_map(move |a| {
            let inner = inner.clone();
            (min.z..=max.z).flat_map(move |z| {
                inner.clone().map(move |b| {
                    if column_major {
                        SectionPos::new(a, b, z)
                    } else {
                        SectionPos::new(b, a, z)
                    }
                })
            })
        })
    }

    /// The number of sections, saturating at `u64::MAX`.
    pub fn len(&self) -> u64 {
        span(self.min.x, self.max.x)
            .saturating_mul(span(self.min.y, self.max.y))
            .saturating_mul(span(self.min.z, self.max.z))
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A vertical extent of whole sections: the world's `min_y` and `height` from its dimension type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ColumnLayout {
    pub min_section_y: i32,
    pub sections: u32,
}

impl ColumnLayout {
    pub const fn min_y(self) -> i32 {
        self.min_section_y.saturating_mul(SectionPos::SIZE as i32)
    }

    pub const fn height(self) -> u32 {
        self.sections.saturating_mul(SectionPos::SIZE as u32)
    }

    pub const fn contains_section_y(self, section_y: i32) -> bool {
        let offset = (section_y as i64).wrapping_sub(self.min_section_y as i64);
        offset >= 0 && offset < self.sections as i64
    }
}

/// The cells of `lo..=hi`; zero when `lo` exceeds `hi`.
pub(crate) fn span(lo: i32, hi: i32) -> u64 {
    u64::try_from(i64::from(hi).wrapping_sub(i64::from(lo)).wrapping_add(1)).unwrap_or(0)
}
