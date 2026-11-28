use core::ops::Range;
use glam::IVec3;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_noise::strata::{AXIS_X, AXIS_Y, AXIS_Z, Axes, run_len};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Window {
    pub grid: SampleGrid,
    pub x: usize,
    pub z: usize,
    pub y: usize,
}

impl Window {
    pub(crate) fn whole(volume: &SampleGrid) -> Self {
        Self {
            grid: *volume,
            x: 0,
            z: 0,
            y: 0,
        }
    }

    pub(crate) fn column(volume: &SampleGrid, x: usize, z: usize, rows: Range<usize>) -> Self {
        let grid = SampleGrid::new(
            IVec3::new(1, rows.len().max(1) as i32, 1),
            IVec3::new(
                volume.block_x(x as i32),
                volume.block_y(rows.start as i32),
                volume.block_z(z as i32),
            ),
            volume.step_block(),
        );
        Self {
            grid,
            x,
            z,
            y: rows.start,
        }
    }

    pub(crate) fn start(&self, axes: Axes, volume: &SampleGrid) -> usize {
        element(axes, volume, self.x, self.z, self.y)
    }

    pub(crate) fn columns(&self) -> impl Iterator<Item = (usize, usize)> {
        let size = self.grid.size();
        let (x, z) = (self.x, self.z);
        (z..z + size.z as usize)
            .flat_map(move |iz| (x..x + size.x as usize).map(move |ix| (ix, iz)))
    }

    pub(crate) fn rows(&self) -> Range<usize> {
        self.y..self.y + self.grid.size().y as usize
    }
}

pub(crate) fn element(axes: Axes, volume: &SampleGrid, x: usize, z: usize, y: usize) -> usize {
    let size = volume.size();
    let has = |bit: Axes| axes & bit != 0;
    let width = if has(AXIS_X) { size.x as usize } else { 1 };
    let x = if has(AXIS_X) { x } else { 0 };
    let z = if has(AXIS_Z) { z } else { 0 };
    let y = if has(AXIS_Y) { y } else { 0 };
    (x + z * width) * run_len(axes, size.y as usize) + y
}
