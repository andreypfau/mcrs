//! Sky light is not a special propagation rule — it is a special *source* rule.
//! A cell is a sky source when nothing occludes the run of column above it, and
//! below the lowest source the sky layer fades exactly like block light.
//!
//! Modelling it the other way round, as a downward step that costs nothing,
//! looks equivalent and is not: light that arrived sideways under an overhang
//! would then fall to the bottom of the world undimmed.

use crate::level::BlockColumn;
use crate::region::BlockBox;

/// Lowest sky-source Y for every block column under a working area.
#[derive(Clone, Debug)]
pub struct SkyFloors {
    min_x: i32,
    min_z: i32,
    width_x: usize,
    lowest_source_y: Vec<i32>,
}

impl SkyFloors {
    pub fn collect(area: BlockBox, mut floor_of: impl FnMut(BlockColumn) -> i32) -> Self {
        let width_x = (area.max.x - area.min.x + 1) as usize;
        let width_z = (area.max.z - area.min.z + 1) as usize;
        let mut lowest_source_y = Vec::with_capacity(width_x * width_z);
        for z in area.min.z..=area.max.z {
            for x in area.min.x..=area.max.x {
                lowest_source_y.push(floor_of(BlockColumn { x, z }));
            }
        }
        Self {
            min_x: area.min.x,
            min_z: area.min.z,
            width_x,
            lowest_source_y,
        }
    }

    /// Lowest Y in this column that is still a sky source.
    pub fn get(&self, x: i32, z: i32) -> i32 {
        let ix = (x - self.min_x) as usize;
        let iz = (z - self.min_z) as usize;
        self.lowest_source_y[iz * self.width_x + ix]
    }

    /// The sky source test: every cell from here up is a source.
    pub fn is_source(&self, x: i32, y: i32, z: i32) -> bool {
        y >= self.get(x, z)
    }
}
