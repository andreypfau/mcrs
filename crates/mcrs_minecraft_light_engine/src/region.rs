use mcrs_minecraft_core::BoundingBox;
use mcrs_voxel::BlockPos;

pub const INFLUENCE_RADIUS: i32 = 15;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SectionErase {
    None,
    All,
    Partial,
}

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

#[derive(Clone, Debug)]
pub enum ErasePlan {
    Everything(BoundingBox),
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

#[derive(Clone, Debug, Default)]
pub struct Regions {
    influences: Vec<Influence>,
}

impl Regions {
    pub fn push(&mut self, influence: Influence) {
        self.influences.push(influence);
    }

    pub fn bounds(&self) -> Option<BoundingBox> {
        self.influences
            .iter()
            .map(Influence::bounds)
            .reduce(BoundingBox::union)
    }

    pub fn erase_plan(self, area_cells: u64, influenced: BoundingBox) -> ErasePlan {
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
