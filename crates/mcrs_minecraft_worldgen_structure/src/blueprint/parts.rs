//! The parts village buildings share: doors, beds, windows, roofs, fences,
//! ground drawn from rows of text, and the jigsaws that join pieces.

use mcrs_minecraft_keys as keys;
use std::collections::BTreeSet;
use std::ops::RangeInclusive;

use mcrs_minecraft_core::{Axis, Direction};

use super::{Canvas, Cell, Gable, block, settled, stairs};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hinge {
    Left,
    Right,
}

/// How the two slopes of a gable are built: at which ends of the ridge
/// upside-down stairs hang under them, and the level each slope starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slopes {
    pub boards: [bool; 2],
    pub low_from: i32,
    pub high_from: i32,
}

impl Slopes {
    pub const BOARDED: Self = Self::boards([true, true]);
    pub const BARE: Self = Self::boards([false, false]);

    pub const fn boards(boards: [bool; 2]) -> Self {
        Slopes {
            boards,
            low_from: 0,
            high_from: 0,
        }
    }

    pub const fn from(self, low_from: i32, high_from: i32) -> Self {
        Slopes {
            boards: self.boards,
            low_from,
            high_from,
        }
    }
}

/// A set of `[x, z]` cells of one layer: a geometric shape, widened and
/// narrowed by the cells that deviate from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    cells: BTreeSet<[i32; 2]>,
}

impl Patch {
    fn of(min: [i32; 2], max: [i32; 2], inside: impl Fn(i32, i32) -> bool) -> Self {
        let cells = (min[0]..=max[0])
            .flat_map(|x| (min[1]..=max[1]).map(move |z| [x, z]))
            .filter(|[x, z]| inside(*x, *z))
            .collect();
        Patch { cells }
    }

    pub fn rectangle(min: [i32; 2], max: [i32; 2]) -> Self {
        Self::of(min, max, |_, _| true)
    }

    /// A rectangle without the cells nearer than `cut` to a corner, counted
    /// in steps along the sides.
    pub fn clipped_rectangle(min: [i32; 2], max: [i32; 2], cut: i32) -> Self {
        Self::of(min, max, |x, z| {
            (x - min[0]).min(max[0] - x) + (z - min[1]).min(max[1] - z) >= cut
        })
    }

    /// The cells within `radius` steps of `centre`, diagonals counting two.
    pub fn diamond(centre: [i32; 2], radius: i32) -> Self {
        let [x, z] = centre;
        Self::of(
            [x - radius, z - radius],
            [x + radius, z + radius],
            |at, to| (at - x).abs() + (to - z).abs() <= radius,
        )
    }

    /// The cells outside the shape that belong to the patch all the same.
    pub fn added(mut self, cells: &[[i32; 2]]) -> Self {
        self.cells.extend(cells);
        self
    }

    /// The cells inside the shape that do not belong to the patch.
    pub fn removed(mut self, cells: &[[i32; 2]]) -> Self {
        for cell in cells {
            self.cells.remove(cell);
        }
        self
    }

    pub fn cells(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        self.cells.iter().copied()
    }
}

/// The values one coordinate takes: a single one, a range, or each of a list
/// of either.
pub trait Coords {
    fn values(&self) -> Vec<i32>;
}

impl Coords for i32 {
    fn values(&self) -> Vec<i32> {
        vec![*self]
    }
}

impl Coords for RangeInclusive<i32> {
    fn values(&self) -> Vec<i32> {
        self.clone().collect()
    }
}

impl<T: Coords, const N: usize> Coords for [T; N] {
    fn values(&self) -> Vec<i32> {
        self.iter().flat_map(Coords::values).collect()
    }
}

/// Upside-down stairs facing `facing`, their shape left to the neighbours.
pub fn top_stairs(stairs_block: &str, facing: Direction) -> Cell {
    let Cell::Settled(state) = stairs(stairs_block, facing) else {
        unreachable!("stairs are settled")
    };
    Cell::Settled(super::rules::with(&state, "half", "top"))
}

/// Bottom stairs whose corner is spelled out, where the neighbours that
/// would turn it are not part of the template.
pub fn corner_stairs(stairs_block: &str, facing: Direction, shape: &str) -> Cell {
    block(&format!(
        "{stairs_block}[facing={},half=bottom,shape={shape},waterlogged=false]",
        facing.name()
    ))
}

/// A fence joined to exactly `sides`, whatever stands next to it.
pub fn fence_joined(fence: &str, sides: &[Direction]) -> Cell {
    use Direction::{East, North, South, West};
    let [east, north, south, west] = [East, North, South, West].map(|side| sides.contains(&side));
    block(&format!(
        "{fence}[east={east},north={north},south={south},waterlogged=false,west={west}]"
    ))
}

/// A wall post joined low to the sides in `low` and tall to those in `tall`,
/// whatever stands next to it.
pub fn wall_joined(wall: &str, low: &[Direction], tall: &[Direction]) -> Cell {
    use Direction::{East, North, South, West};
    let [east, north, south, west] = [East, North, South, West].map(|side| {
        if tall.contains(&side) {
            "tall"
        } else if low.contains(&side) {
            "low"
        } else {
            "none"
        }
    });
    block(&format!(
        "{wall}[east={east},north={north},south={south},up=true,waterlogged=false,west={west}]"
    ))
}

impl Canvas {
    fn door_half(&mut self, door: &str, at: [i32; 3], facing: Direction, hinge: Hinge, open: bool) {
        let hinge = match hinge {
            Hinge::Left => "left",
            Hinge::Right => "right",
        };
        let lower = block(&format!(
            "{door}[facing={},half=lower,hinge={hinge},open={open},powered=false]",
            facing.name()
        ));
        self.place(&lower, at[0], at[1], at[2]);
    }

    /// The lower half; the rule layer adds the upper one.
    pub fn door(&mut self, door: &str, at: [i32; 3], facing: Direction, hinge: Hinge) {
        self.door_half(door, at, facing, hinge, false);
    }

    /// A door left standing open.
    pub fn open_door(&mut self, door: &str, at: [i32; 3], facing: Direction, hinge: Hinge) {
        self.door_half(door, at, facing, hinge, true);
    }

    /// The foot; the head lies one block further along `facing`.
    pub fn bed(&mut self, bed: &str, foot: [i32; 3], facing: Direction) {
        let foot_part = block(&format!(
            "{bed}[facing={},occupied=false,part=foot]",
            facing.name()
        ));
        self.place(&foot_part, foot[0], foot[1], foot[2]);
    }

    /// A pane with `flank` on both sides along the wall.
    pub fn window(&mut self, pane: &Cell, flank: Option<&Cell>, at: [i32; 3], along: Axis) {
        let [x, y, z] = at;
        self.place(pane, x, y, z);
        if let Some(flank) = flank {
            let (dx, dz) = if along == Axis::X { (1, 0) } else { (0, 1) };
            self.place(flank, x - dx, y, z - dz);
            self.place(flank, x + dx, y, z + dz);
        }
    }

    /// `block` at every pairing of the values the three coordinates take.
    pub fn fill(&mut self, block: &Cell, xs: impl Coords, ys: impl Coords, zs: impl Coords) {
        let (ys, zs) = (ys.values(), zs.values());
        for x in xs.values() {
            for &y in &ys {
                for &z in &zs {
                    self.place(block, x, y, z);
                }
            }
        }
    }

    /// `block` at each of `cells`.
    pub fn each(&mut self, block: &Cell, cells: &[[i32; 3]]) {
        for [x, y, z] in cells {
            self.place(block, *x, *y, *z);
        }
    }

    /// A box of `block` between each pair of corners.
    pub fn boxes(&mut self, block: &Cell, boxes: &[([i32; 3], [i32; 3])]) {
        for (min, max) in boxes {
            self.solid(block, *min, *max);
        }
    }

    /// A column of `post` from `y[0]` to `y[1]` at every pairing of `xs` and `zs`.
    pub fn posts(&mut self, post: &Cell, xs: &[i32], zs: &[i32], y: [i32; 2]) {
        for &x in xs {
            for &z in zs {
                self.solid(post, [x, y[0], z], [x, y[1], z]);
            }
        }
    }

    /// A column of `post` with a torch standing on it.
    pub fn torch_post(&mut self, post: &Cell, at: [i32; 3], height: i32) {
        let [x, y, z] = at;
        self.solid(post, [x, y, z], [x, y + height - 1, z]);
        self.place(&block(keys::block::TORCH.as_static_str()), x, y + height, z);
    }

    /// A fence round a rectangle, with a gate in place of the fence at each of `gates`.
    pub fn fence_ring(
        &mut self,
        fence: &Cell,
        gate: &Cell,
        min: [i32; 3],
        max: [i32; 2],
        gates: &[[i32; 2]],
    ) {
        self.walls(fence, fence, min, [max[0], min[1], max[1]]);
        for [x, z] in gates {
            self.place(gate, *x, min[1], *z);
        }
    }

    /// `block` on every cell of `patch` that lies inside the box.
    pub fn patch(&mut self, block: &Cell, y: i32, patch: &Patch) {
        let [width, _, depth] = self.size();
        for [x, z] in patch.cells() {
            if (0..width).contains(&x) && (0..depth).contains(&z) {
                self.place(block, x, y, z);
            }
        }
    }

    /// A run of `block` on each successive row south of `z`, between the two
    /// x extents given for that row.
    pub fn rows(&mut self, block: &Cell, y: i32, z: i32, extents: &[[i32; 2]]) {
        for (row, [from, to]) in extents.iter().enumerate() {
            let z = z + row as i32;
            self.solid(block, [*from, y, z], [*to, y, z]);
        }
    }

    /// A run of `block` for each `[from, to, z]`: the x extent and its row.
    pub fn runs(&mut self, block: &Cell, y: i32, runs: &[[i32; 3]]) {
        for [from, to, z] in runs {
            self.solid(block, [*from, y, *z], [*to, y, *z]);
        }
    }

    /// `block` at each `[x, z]` of one layer.
    pub fn scatter(&mut self, block: &Cell, y: i32, cells: &[[i32; 2]]) {
        for [x, z] in cells {
            self.place(block, *x, y, *z);
        }
    }

    /// Rows of stairs climbing from both eaves, `ridge` between the top pair
    /// of an odd span, and the boards under them.
    pub fn roof(&mut self, gable: Gable, stairs_block: &str, ridge: Option<&Cell>, slopes: Slopes) {
        let (low, high) = if gable.ridge == Axis::Z {
            (Direction::East, Direction::West)
        } else {
            (Direction::South, Direction::North)
        };
        let at = |u: i32, y: i32, r: i32| {
            if gable.ridge == Axis::Z {
                [u, y, r]
            } else {
                [r, y, u]
            }
        };
        let [first, last] = gable.length;
        let ends: Vec<i32> = [first, last]
            .into_iter()
            .zip(slopes.boards)
            .filter_map(|(end, boarded)| boarded.then_some(end))
            .collect();
        for i in 0..gable.levels {
            let (a, b, y) = (gable.span[0] + i, gable.span[1] - i, gable.y + i);
            if a >= b {
                break;
            }
            let sides = [
                (a, a + 1, low, high, slopes.low_from),
                (b, b - 1, high, low, slopes.high_from),
            ];
            for (row, under, facing, board, from) in sides {
                if i < from {
                    continue;
                }
                self.solid(
                    &stairs(stairs_block, facing),
                    at(row, y, first),
                    at(row, y, last),
                );
                if a + 2 < b {
                    for &end in &ends {
                        let [x, y, z] = at(under, y, end);
                        self.place(&top_stairs(stairs_block, board), x, y, z);
                    }
                }
            }
            if a + 2 == b
                && let Some(ridge) = ridge
            {
                self.solid(ridge, at(a + 1, y, first), at(a + 1, y, last));
            }
        }
    }

    /// A jigsaw whose orientation follows from the face it stands on.
    pub fn socket(&mut self, at: [i32; 3], name: &str, pool: &str, final_state: &str) {
        self.place(
            &settled(keys::block::JIGSAW.as_static_str()),
            at[0],
            at[1],
            at[2],
        );
        self.jigsaw(at, name, pool, final_state);
    }

    /// A jigsaw with its orientation stated.
    pub fn socket_facing(
        &mut self,
        at: [i32; 3],
        orientation: &str,
        name: &str,
        pool: &str,
        final_state: &str,
    ) {
        let jigsaw = block(&format!("minecraft:jigsaw[orientation={orientation}]"));
        self.place(&jigsaw, at[0], at[1], at[2]);
        self.jigsaw(at, name, pool, final_state);
    }

    /// Where a street meets the building, turning into `step` once joined.
    pub fn entrance(&mut self, at: [i32; 3], pool: &str, step: &str) {
        self.socket(at, "minecraft:building_entrance", pool, step);
    }

    /// Where a piece of `pool` stands, on a block that turns into `floor`.
    pub fn spot(&mut self, at: [i32; 3], pool: &str, floor: &str) {
        self.socket_facing(at, "up_north", "minecraft:bottom", pool, floor);
    }

    /// The end of a street that nothing joins.
    pub fn dead_end(&mut self, at: [i32; 3]) {
        self.socket_facing(
            at,
            "east_up",
            "minecraft:street",
            mcrs_minecraft_worldgen_feature::keys::template_pool::EMPTY.as_str(),
            keys::block::STRUCTURE_VOID.as_static_str(),
        );
    }

    /// A layer that holds jigsaws and is otherwise no part of the template.
    pub fn jigsaw_layer(&mut self, y: i32) {
        let [width, _, depth] = self.size();
        self.void([0, y, 0], [width - 1, y, depth - 1]);
    }
}
