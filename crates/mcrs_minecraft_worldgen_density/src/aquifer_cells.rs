use crate::aquifer::{FluidStatus, X_SPACING_SHIFT, Y_SPACING, anchor, flat};
use crate::router::NoiseRouter;
use crate::router_key::RouterKey;
use glam::IVec3;
use mcrs_minecraft_random::{Random, RandomSource};

#[derive(Default, Debug)]
pub struct AquiferCells {
    owner: Option<RouterKey>,
    min_grid: IVec3,
    max_grid: IVec3,
    size: IVec3,
    centres: Vec<IVec3>,
    statuses: Vec<Option<FluidStatus>>,
}

impl AquiferCells {
    pub fn cover(&mut self, router: &NoiseRouter, min: IVec3, max: IVec3) {
        let Some(config) = router.aquifer.as_ref() else {
            return;
        };
        let min_grid = anchor(min.x, min.y, min.z) - IVec3::Y;
        let max_grid = anchor(max.x, max.y, max.z) + IVec3::ONE;
        let owner = router.key();
        if self.owner == Some(owner)
            && self.min_grid.cmple(min_grid).all()
            && self.max_grid.cmpge(max_grid).all()
        {
            return;
        }
        self.owner = Some(owner);
        self.min_grid = min_grid;
        self.max_grid = max_grid;
        self.size = max_grid - min_grid + IVec3::ONE;
        self.centres.clear();
        for gy in min_grid.y..=max_grid.y {
            for gz in min_grid.z..=max_grid.z {
                for gx in min_grid.x..=max_grid.x {
                    self.centres.push(centre(&config.random, gx, gy, gz));
                }
            }
        }
        self.statuses.clear();
        self.statuses.resize(self.centres.len(), None);
    }

    #[inline]
    pub(crate) fn index(&self, grid: IVec3) -> usize {
        flat(grid - self.min_grid, self.size)
    }

    #[inline]
    pub(crate) fn centre(&self, index: usize) -> Option<IVec3> {
        self.centres.get(index).copied()
    }

    #[inline]
    pub(crate) fn status(&self, index: usize) -> Option<FluidStatus> {
        self.statuses.get(index).copied().flatten()
    }

    pub(crate) fn put(&mut self, index: usize, status: FluidStatus) {
        if let Some(slot) = self.statuses.get_mut(index) {
            *slot = Some(status);
        }
    }
}

fn centre(random: &RandomSource, gx: i32, gy: i32, gz: i32) -> IVec3 {
    let mut random = random.clone().fork_at(IVec3::new(gx, gy, gz));
    let jx = random.next_i32_bound(10);
    let jy = random.next_i32_bound(9);
    let jz = random.next_i32_bound(10);
    IVec3::new(
        (gx << X_SPACING_SHIFT) + jx,
        gy * Y_SPACING + jy,
        (gz << X_SPACING_SHIFT) + jz,
    )
}
