use mcrs_voxel_math::{BlockPos, ChunkPos};

use crate::level::{LightBounds, SECTION_WIDTH};

/// Maximum distance a single change can travel: every step costs at least one
/// level and light never exceeds 15, so cells beyond this are still correct and
/// serve as a fixed boundary.
pub const INFLUENCE_RADIUS: i32 = 15;

/// An inclusive box in block coordinates.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct BlockBox {
    pub min: BlockPos,
    pub max: BlockPos,
}

impl BlockBox {
    pub fn point(pos: BlockPos) -> Self {
        Self { min: pos, max: pos }
    }

    pub fn of_section(pos: ChunkPos) -> Self {
        let min = BlockPos::new(
            pos.x * SECTION_WIDTH,
            pos.y * SECTION_WIDTH,
            pos.z * SECTION_WIDTH,
        );
        Self {
            min,
            max: BlockPos::new(
                min.x + SECTION_WIDTH - 1,
                min.y + SECTION_WIDTH - 1,
                min.z + SECTION_WIDTH - 1,
            ),
        }
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: BlockPos::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: BlockPos::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    pub fn expand(self, amount: i32) -> Self {
        Self {
            min: BlockPos::new(
                self.min.x - amount,
                self.min.y - amount,
                self.min.z - amount,
            ),
            max: BlockPos::new(
                self.max.x + amount,
                self.max.y + amount,
                self.max.z + amount,
            ),
        }
    }

    pub fn clamp_vertically(self, bounds: LightBounds) -> Self {
        Self {
            min: BlockPos::new(self.min.x, self.min.y.max(bounds.min_light_y()), self.min.z),
            max: BlockPos::new(self.max.x, self.max.y.min(bounds.max_light_y()), self.max.z),
        }
    }

    pub fn cells(self) -> u64 {
        let dx = (self.max.x - self.min.x + 1).max(0) as u64;
        let dy = (self.max.y - self.min.y + 1).max(0) as u64;
        let dz = (self.max.z - self.min.z + 1).max(0) as u64;
        dx.saturating_mul(dy).saturating_mul(dz)
    }

    /// Smallest section-aligned box containing this one: the shape a working
    /// field really takes, and so the shape a batch has to be measured by.
    pub fn section_aligned(self) -> Self {
        let low = |v: i32| v.div_euclid(SECTION_WIDTH) * SECTION_WIDTH;
        Self {
            min: BlockPos::new(low(self.min.x), low(self.min.y), low(self.min.z)),
            max: BlockPos::new(
                low(self.max.x) + SECTION_WIDTH - 1,
                low(self.max.y) + SECTION_WIDTH - 1,
                low(self.max.z) + SECTION_WIDTH - 1,
            ),
        }
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
            && self.min.z <= other.max.z
            && other.min.z <= self.max.z
    }

    fn excess(value: i32, low: i32, high: i32) -> i32 {
        if value < low {
            low - value
        } else if value > high {
            value - high
        } else {
            0
        }
    }

    /// L1 distance from a point to the nearest cell of the box.
    pub fn distance_to(self, pos: BlockPos) -> i32 {
        Self::excess(pos.x, self.min.x, self.max.x)
            + Self::excess(pos.y, self.min.y, self.max.y)
            + Self::excess(pos.z, self.min.z, self.max.z)
    }

    pub fn contains(self, pos: BlockPos) -> bool {
        self.distance_to(pos) == 0
    }

    pub fn contains_box(self, other: Self) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && self.min.z <= other.min.z
            && other.max.x <= self.max.x
            && other.max.y <= self.max.y
            && other.max.z <= self.max.z
    }

    pub fn corners(self) -> [BlockPos; 8] {
        let (lo, hi) = (self.min, self.max);
        [
            BlockPos::new(lo.x, lo.y, lo.z),
            BlockPos::new(hi.x, lo.y, lo.z),
            BlockPos::new(lo.x, hi.y, lo.z),
            BlockPos::new(hi.x, hi.y, lo.z),
            BlockPos::new(lo.x, lo.y, hi.z),
            BlockPos::new(hi.x, lo.y, hi.z),
            BlockPos::new(lo.x, hi.y, hi.z),
            BlockPos::new(hi.x, hi.y, hi.z),
        ]
    }
}

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
    pub core: BlockBox,
}

impl Influence {
    pub fn new(core: BlockBox) -> Self {
        Self { core }
    }

    pub fn around(pos: BlockPos) -> Self {
        Self::new(BlockBox::point(pos))
    }

    pub fn contains(&self, pos: BlockPos) -> bool {
        self.core.distance_to(pos) <= INFLUENCE_RADIUS
    }

    pub fn bounds(&self) -> BlockBox {
        self.core.expand(INFLUENCE_RADIUS)
    }
}

/// How the working area should be cleared before it is filled again.
#[derive(Clone, Debug)]
pub enum ErasePlan {
    /// Clear everything except the outer shell. Cheaper once the influences
    /// between them cover most of the area anyway — on a bulk edit, building
    /// the precise union costs several times more than the calculation it saves.
    Everything(BlockBox),
    /// Clear only cells inside some influence.
    Selective(Regions),
}

impl ErasePlan {
    pub fn covers(&self, pos: BlockPos) -> bool {
        match self {
            ErasePlan::Everything(bounds) => bounds.contains(pos),
            ErasePlan::Selective(regions) => regions.contains(pos),
        }
    }

    pub fn meets(&self, section: BlockBox) -> SectionErase {
        match self {
            ErasePlan::Everything(bounds) => {
                if bounds.contains_box(section) {
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
    pub fn bounds(&self) -> Option<BlockBox> {
        self.influences
            .iter()
            .map(Influence::bounds)
            .reduce(BlockBox::union)
    }

    /// Decides how to clear an area of `area_cells` whose changeable part is
    /// `influenced`, and packages the data that plan needs.
    pub fn erase_plan(self, area_cells: u64, influenced: BlockBox) -> ErasePlan {
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
    fn meets(&self, section: BlockBox) -> SectionErase {
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
