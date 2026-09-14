use mcrs_voxel_math::{BlockPos, BoundingBox};

/// Maximum distance a single change can travel: every step costs at least one
/// level and light never exceeds 15, so cells beyond this are still correct and
/// serve as a fixed boundary.
pub const INFLUENCE_RADIUS: i32 = 15;

/// How an erase plan meets one section: the answer is the same for all 4096
/// cells except on the fringe, so the per-cell test is worth avoiding.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SectionErase {
    None,
    All,
    Partial,
}

/// The region one edit can change: an L1 dilation of the box where the edit
/// altered either the emission or the attenuation.
///
/// For a plain block change the core is a single cell, so this is an L1 ball.
/// For an edit that moves the sky boundary the core is the vertical run of
/// column whose source flag flipped, which is why the dilation is of a *box*
/// and not of a point: the fifteen-cell caps above and below that run are part
/// of the affected set, and leaving the lower one out strands a lit column
/// under water or under leaves.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Influence {
    pub core: BoundingBox,
}

impl Influence {
    pub fn new(core: BoundingBox) -> Self {
        Self { core }
    }

    pub fn around(pos: BlockPos) -> Self {
        Self::new(BoundingBox::point(pos))
    }

    pub fn contains(&self, pos: BlockPos) -> bool {
        self.core.distance_to(pos) <= INFLUENCE_RADIUS
    }

    pub fn bounds(&self) -> BoundingBox {
        self.core.inflated(INFLUENCE_RADIUS)
    }
}

/// How the working area should be cleared before it is filled again.
#[derive(Clone, Debug)]
pub enum ErasePlan {
    /// Clear everything except the outer shell. Cheaper once the influences
    /// between them cover most of the area anyway — on a bulk edit, building
    /// the precise union costs several times more than the calculation it saves.
    Everything(BoundingBox),
    /// Clear only cells inside some influence.
    Selective(Regions),
}

impl ErasePlan {
    pub fn covers(&self, pos: BlockPos) -> bool {
        match self {
            ErasePlan::Everything(bounds) => bounds.is_inside(pos),
            ErasePlan::Selective(regions) => regions.contains(pos),
        }
    }

    pub fn meets(&self, section: BoundingBox) -> SectionErase {
        match self {
            ErasePlan::Everything(bounds) => {
                if bounds.contains(section) {
                    SectionErase::All
                } else if bounds.intersects(section) {
                    SectionErase::Partial
                } else {
                    SectionErase::None
                }
            }
            ErasePlan::Selective(regions) => regions.meets(section),
        }
    }
}

/// The influences of one batch of edits, merged into a single working area.
#[derive(Clone, Debug, Default)]
pub struct Regions {
    influences: Vec<Influence>,
}

impl Regions {
    pub fn push(&mut self, influence: Influence) {
        self.influences.push(influence);
    }

    /// Bounding box of everything that can change. The working area is this
    /// box plus a one-cell shell of untouched, and therefore already correct,
    /// values that hold the calculation in place.
    pub fn bounds(&self) -> Option<BoundingBox> {
        self.influences
            .iter()
            .map(Influence::bounds)
            .reduce(BoundingBox::union)
    }

    /// Decides how to clear an area of `area_cells` whose changeable part is
    /// `influenced`, and packages the data that plan needs.
    pub fn erase_plan(self, area_cells: u64, influenced: BoundingBox) -> ErasePlan {
        // The bounding box of each dilation, not the dilation itself: an L1
        // ball fills about a sixth of its box, but for a large core the box is
        // close to tight, and it is the large cores that decide the question.
        let marking_cost: u64 = self
            .influences
            .iter()
            .map(|influence| influence.bounds().cells())
            .sum();
        if marking_cost >= area_cells {
            ErasePlan::Everything(influenced)
        } else {
            ErasePlan::Selective(self)
        }
    }

    fn contains(&self, pos: BlockPos) -> bool {
        self.influences.iter().any(|i| i.contains(pos))
    }

    /// An influence covers a convex L1 ball, so a box lies wholly inside one
    /// exactly when its eight corners do.
    fn meets(&self, section: BoundingBox) -> SectionErase {
        let mut touched = false;
        for influence in &self.influences {
            if !influence.bounds().intersects(section) {
                continue;
            }
            if section.corners().iter().all(|&c| influence.contains(c)) {
                return SectionErase::All;
            }
            touched = true;
        }
        if touched {
            SectionErase::Partial
        } else {
            SectionErase::None
        }
    }
}
