use core::num::NonZeroU32;

/// The key of a chunk in a world, in chunks on each axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// The position of a cell in the cells of one world, from 0 up to the cell count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellIndex(pub u32);

/// The minimum leaf size of one axis: a size in chunks, or the whole axis, which the partition
/// never divides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    Size(NonZeroU32),
    Whole,
}

/// The minimum leaf size on each axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeafSize {
    pub x: Leaf,
    pub y: Leaf,
    pub z: Leaf,
}

/// A static division of the chunks of one world into boxes of the leaf size. Each box belongs to
/// one cell, and the boxes of the cells repeat in a grid, so neighbour boxes belong to different
/// cells when the count allows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoxPartition {
    leaf: LeafSize,
    cells: NonZeroU32,
    grid: Grid,
}

/// The extent of the grid of cells on each axis. The product is the cell count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Grid {
    x: u32,
    y: u32,
    z: u32,
}

impl BoxPartition {
    /// Divides a world into boxes of `leaf` over `cells` cells. An axis with a [`Leaf::Whole`]
    /// leaf takes no part in the grid; with no axis to divide, one cell owns everything.
    #[must_use]
    pub fn new(leaf: LeafSize, cells: NonZeroU32) -> Self {
        let sized = [leaf.x, leaf.z, leaf.y].map(|axis| matches!(axis, Leaf::Size(_)));
        let mut axes_left = sized.iter().filter(|sized| **sized).count();
        let mut remaining = cells.get();
        let mut extents = [1u32; 3];
        for (extent, sized) in extents.iter_mut().zip(sized) {
            if !sized {
                continue;
            }
            let extent_here = if axes_left <= 1 {
                remaining
            } else {
                smallest_root_divisor(remaining, axes_left)
            };
            *extent = extent_here;
            remaining = remaining.checked_div(extent_here).unwrap_or(1);
            axes_left = axes_left.saturating_sub(1);
        }
        let [x, z, y] = extents;
        let cells = if x.saturating_mul(y).saturating_mul(z) == cells.get() {
            cells
        } else {
            NonZeroU32::MIN
        };
        Self {
            leaf,
            cells,
            grid: Grid { x, y, z },
        }
    }

    /// The number of cells.
    #[must_use]
    pub fn cells(&self) -> NonZeroU32 {
        self.cells
    }

    /// The cell that owns `key`.
    #[must_use]
    pub fn owner(&self, key: ChunkKey) -> CellIndex {
        self.owner_of_box(self.box_of(key))
    }

    /// Whether the cell `index` owns a chunk within `reach` chunks of `key` on every axis.
    #[must_use]
    pub fn reaches(&self, index: CellIndex, key: ChunkKey, reach: u32) -> bool {
        let reach = i32::try_from(reach).unwrap_or(i32::MAX);
        let low = self.box_of(ChunkKey {
            x: key.x.saturating_sub(reach),
            y: key.y.saturating_sub(reach),
            z: key.z.saturating_sub(reach),
        });
        let high = self.box_of(ChunkKey {
            x: key.x.saturating_add(reach),
            y: key.y.saturating_add(reach),
            z: key.z.saturating_add(reach),
        });
        // Past one grid period on an axis, every cell of that axis is in reach.
        let span = |low: i32, high: i32, extent: u32| {
            let count = i64::from(high) - i64::from(low) + 1;
            low..=low.saturating_add(i32::try_from(count.min(i64::from(extent))).unwrap_or(1) - 1)
        };
        span(low.x, high.x, self.grid.x).any(|x| {
            span(low.y, high.y, self.grid.y).any(|y| {
                span(low.z, high.z, self.grid.z)
                    .any(|z| self.owner_of_box(ChunkKey { x, y, z }) == index)
            })
        })
    }

    fn box_of(&self, key: ChunkKey) -> ChunkKey {
        ChunkKey {
            x: box_coordinate(key.x, self.leaf.x),
            y: box_coordinate(key.y, self.leaf.y),
            z: box_coordinate(key.z, self.leaf.z),
        }
    }

    fn owner_of_box(&self, boxed: ChunkKey) -> CellIndex {
        let along = |coordinate: i32, extent: u32| {
            let extent = i64::from(extent.max(1));
            u32::try_from(i64::from(coordinate).rem_euclid(extent)).unwrap_or(0)
        };
        let x = along(boxed.x, self.grid.x);
        let z = along(boxed.z, self.grid.z);
        let y = along(boxed.y, self.grid.y);
        CellIndex(x + self.grid.x * (z + self.grid.z * y))
    }
}

fn box_coordinate(coordinate: i32, leaf: Leaf) -> i32 {
    match leaf {
        Leaf::Size(size) => {
            let size = i64::from(size.get());
            i32::try_from(i64::from(coordinate).div_euclid(size)).unwrap_or(0)
        }
        Leaf::Whole => 0,
    }
}

/// The smallest divisor `d` of `count` with `d` to the power `axes` at least `count`, so the
/// grid is as close to a cube as the count allows.
fn smallest_root_divisor(count: u32, axes: usize) -> u32 {
    let axes = u32::try_from(axes).unwrap_or(1);
    (1..=count)
        .find(|divisor| {
            count.is_multiple_of(*divisor)
                && divisor.checked_pow(axes).is_none_or(|power| power >= count)
        })
        .unwrap_or(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(cells: u32) -> BoxPartition {
        let size = Leaf::Size(NonZeroU32::new(32).unwrap());
        BoxPartition::new(
            LeafSize {
                x: size,
                y: Leaf::Whole,
                z: size,
            },
            NonZeroU32::new(cells).unwrap(),
        )
    }

    fn key(x: i32, z: i32) -> ChunkKey {
        ChunkKey { x, y: 0, z }
    }

    #[test]
    fn the_four_boxes_at_a_corner_belong_to_four_cells() {
        let partition = columns(4);
        let mut owners: Vec<CellIndex> = [key(-1, -1), key(0, -1), key(-1, 0), key(0, 0)]
            .into_iter()
            .map(|key| partition.owner(key))
            .collect();
        owners.sort_unstable();
        owners.dedup();
        assert_eq!(owners.len(), 4);
        assert_eq!(partition.owner(key(0, 0)), partition.owner(key(31, 31)));
        assert_eq!(partition.owner(key(0, 0)), partition.owner(key(64, 64)));
    }

    #[test]
    fn a_cell_reaches_a_chunk_only_within_the_reach_of_its_boxes() {
        let partition = columns(2);
        let west = partition.owner(key(-1, 0));
        assert_ne!(west, partition.owner(key(0, 0)));
        assert!(partition.reaches(west, key(2, 5), 3));
        assert!(!partition.reaches(west, key(3, 5), 2));
        assert!(partition.reaches(west, key(-20, 5), 0));
    }

    #[test]
    fn a_world_with_nothing_to_divide_has_one_owner() {
        let partition = BoxPartition::new(
            LeafSize {
                x: Leaf::Whole,
                y: Leaf::Whole,
                z: Leaf::Whole,
            },
            NonZeroU32::new(3).unwrap(),
        );
        assert_eq!(partition.cells().get(), 1);
        assert_eq!(partition.owner(key(500, -500)), CellIndex(0));
    }
}
