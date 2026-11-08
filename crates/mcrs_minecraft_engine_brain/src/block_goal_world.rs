use glam::DVec3;
use glam::IVec3;
use mcrs_minecraft_brain::BlockPos;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_goals::BuildHeight;
use mcrs_minecraft_goals::CellFlags;
use mcrs_minecraft_goals::GoalWorld;
use mcrs_minecraft_goals::GroundCell;
use mcrs_minecraft_volume::BlockVolume;
use mcrs_minecraft_volume::Unavailable;

use crate::CellBox;
use crate::CellTable;
use crate::PathType;
use crate::StateCell;
use crate::StateFlags;

/// The walk value above `grass_block`, and elsewhere on a server without a light engine
/// (GOAL-POS-006).
const GRASS_WALK_VALUE: f32 = 10.0;
const OTHER_WALK_VALUE: f32 = 0.5;
/// The longest line of sight (GOAL-QRY-003).
const SIGHT_RANGE: f64 = 128.0;

/// The blocks of a dimension as the goals of a mob of width 1 read them.
#[derive(Debug)]
pub struct BlockGoalWorld<'a, V> {
    blocks: &'a V,
    cells: &'a CellTable,
    height: BuildHeight,
}

impl<'a, V: BlockVolume> BlockGoalWorld<'a, V> {
    #[must_use]
    pub fn new(blocks: &'a V, cells: &'a CellTable) -> Self {
        let bounds = blocks.bounds();
        Self {
            blocks,
            cells,
            height: BuildHeight {
                min: bounds.min.y,
                max: bounds.max.y,
            },
        }
    }

    /// The cell of the state at `at`: air outside the volume, and a blocked cell in a section
    /// that is not loaded.
    #[must_use]
    pub fn state_cell(&self, at: BlockPos) -> StateCell {
        match self
            .blocks
            .block(mcrs_voxel::BlockPos::new(at.x, at.y, at.z))
        {
            Ok(state) => self.cells.state(state),
            Err(Unavailable::Outside) => StateCell::AIR,
            Err(Unavailable::Unloaded | Unavailable::Foreign) => StateCell::UNLOADED,
        }
    }

    /// The top of the collision shape of the block `at` for the move control, in world
    /// coordinates (PATH-MOV-002).
    #[must_use]
    pub fn feet_top(&self, at: BlockPos) -> Option<f64> {
        let top = self.cells.top(self.state_cell(at))?;
        Some(f64::from(at.y) + top)
    }

    /// The block path type at `at` (PATH-TYP-003).
    fn block_path_type(&self, at: BlockPos, here: StateCell, below: StateCell) -> PathType {
        if here.path != PathType::Open || at.y <= self.height.min {
            return here.path;
        }
        if let Some(path) = PathType::over(below.path) {
            return path;
        }
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if dx == 0 && dz == 0 {
                        continue;
                    }
                    let neighbour = self.state_cell(at.offset(dx, dy, dz)).path;
                    if let Some(path) = neighbour.as_neighbour() {
                        return path;
                    }
                }
            }
        }
        PathType::Walkable
    }

    fn blocks_sight(&self, cell: IVec3, start: DVec3, delta: DVec3) -> bool {
        let state = self.state_cell(BlockPos::new(cell.x, cell.y, cell.z));
        let origin = cell.as_dvec3();
        self.cells
            .boxes(state.collision)
            .iter()
            .any(|cell_box| segment_hits(start - origin, delta, cell_box))
    }
}

impl<V: BlockVolume> GoalWorld for BlockGoalWorld<'_, V> {
    fn build_height(&self) -> BuildHeight {
        self.height
    }

    fn cell(&self, at: BlockPos) -> GroundCell {
        let here = self.state_cell(at);
        let below = self.state_cell(at.below());
        let marks = [
            (
                below.flags.contains(StateFlags::SOLID_RENDER),
                CellFlags::STABLE_FLOOR,
            ),
            (
                here.flags.contains(StateFlags::LEGACY_SOLID),
                CellFlags::LEGACY_SOLID,
            ),
            (here.flags.contains(StateFlags::WATER), CellFlags::WATER),
            (
                self.cells.boxes(here.collision).is_empty(),
                CellFlags::NO_COLLISION,
            ),
        ];
        let flags = marks
            .into_iter()
            .filter(|(set, _)| *set)
            .fold(CellFlags::NONE, |flags, (_, flag)| flags.union(flag));
        GroundCell {
            flags,
            malus: self.block_path_type(at, here, below).default_malus(),
            walk_value: if below.flags.contains(StateFlags::GRASS_BLOCK) {
                GRASS_WALK_VALUE
            } else {
                OTHER_WALK_VALUE
            },
        }
    }

    /// Walks the blocks that the segment crosses, in order, and tests the boxes of each one.
    fn line_of_sight(&self, from: Point, to: Point) -> bool {
        let start = DVec3::new(from.x, from.y, from.z);
        let delta = DVec3::new(to.x, to.y, to.z) - start;
        if delta.length_squared() > SIGHT_RANGE * SIGHT_RANGE {
            return false;
        }
        let mut cell = start.floor().as_ivec3();
        let last = (start + delta).floor().as_ivec3();
        let step = IVec3::new(sign(delta.x), sign(delta.y), sign(delta.z));
        let inverse = delta.abs().recip();
        let first_edge = |position: f64, delta: f64, inverse: f64| {
            if delta == 0.0 {
                f64::INFINITY
            } else if delta > 0.0 {
                (position.floor() + 1.0 - position) * inverse
            } else {
                (position - position.floor()) * inverse
            }
        };
        let mut next = DVec3::new(
            first_edge(start.x, delta.x, inverse.x),
            first_edge(start.y, delta.y, inverse.y),
            first_edge(start.z, delta.z, inverse.z),
        );
        let blocks_crossed = (last - cell).abs().element_sum();
        for _ in 0..=blocks_crossed {
            if self.blocks_sight(cell, start, delta) {
                return false;
            }
            if cell == last {
                break;
            }
            if next.x <= next.y && next.x <= next.z {
                cell.x = cell.x.saturating_add(step.x);
                next.x += inverse.x;
            } else if next.y <= next.z {
                cell.y = cell.y.saturating_add(step.y);
                next.y += inverse.y;
            } else {
                cell.z = cell.z.saturating_add(step.z);
                next.z += inverse.z;
            }
        }
        true
    }
}

fn sign(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

/// True when the segment from `start` to `start + delta`, relative to the block, meets the box.
fn segment_hits(start: DVec3, delta: DVec3, cell_box: &CellBox) -> bool {
    let min = DVec3::from_array(cell_box.min);
    let max = DVec3::from_array(cell_box.max);
    let mut near = 0.0_f64;
    let mut far = 1.0_f64;
    for ((start, delta), (min, max)) in start
        .to_array()
        .into_iter()
        .zip(delta.to_array())
        .zip(min.to_array().into_iter().zip(max.to_array()))
    {
        if delta == 0.0 {
            if start < min || start > max {
                return false;
            }
            continue;
        }
        let low = (min - start) / delta;
        let high = (max - start) / delta;
        near = near.max(low.min(high));
        far = far.min(low.max(high));
        if near > far {
            return false;
        }
    }
    true
}
