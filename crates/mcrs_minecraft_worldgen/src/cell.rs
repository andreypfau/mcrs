use crate::bounds::{Bounds, node_bounds};
use crate::interval::Interval;
use crate::jmath::{lerp, mul_add};
use crate::program::{Node, NodeId, Program};
use bevy_math::IVec3;

/// The lattice a router with no `interpolated` node is still asked about.
const DEFAULT_CELL: IVec3 = IVec3::new(4, 8, 4);

/// Margin a whole-cell decision keeps away from zero. [`CellBounds::eval`] is
/// f32 interval arithmetic without outward rounding, so a bound that lands
/// exactly on zero is not trustworthy; cells inside the margin go block by block.
pub const CELL_BOUNDS_SLACK: f32 = 1e-5;

/// The range of the values the fill will actually write inside one cell, given
/// that cell's eight lattice corners.
///
/// Tighter than the corner hull, and for the reason the hull is loose: the
/// blocks reach only `alpha in {0 ..= (cell-1)/cell}` on each axis, because the
/// far face is the next cell's first row. The interpolant is multilinear in x
/// and z and a monotone ramp in y, so over that range its extrema sit at the
/// four corner columns' ends, and the arithmetic here mirrors
/// `interpolate_cells` step for step — the same lerps at the same alphas, the
/// same repeated addition down the ramp.
///
/// The result is then widened by the ramp's own accumulated rounding. The
/// extrema are at the corner columns in exact arithmetic, but the fill reaches a
/// row by adding `step` repeatedly, and an interior column's accumulation rounds
/// differently from a corner one's; `cell.y` additions carry a relative error
/// under `cell.y * EPSILON`, and the margin is twice that.
///
/// That margin rests on the arithmetic, not on a sweep: without it the
/// overshoot is around one part in ten million and turns up in roughly one cell
/// in twenty-five million, which a test that walked a few million would pass by
/// luck. Do not delete it because the sweep stays green.
pub fn sampled_range(cell: IVec3, corner: impl Fn(i32, i32, i32) -> f32) -> Interval {
    let inv_xz = 1.0 / cell.x as f32;
    let inv_y = 1.0 / cell.y as f32;
    let (c000, c001) = (corner(0, 0, 0), corner(0, 0, 1));
    let (c100, c101) = (corner(1, 0, 0), corner(1, 0, 1));
    let (c010, c011) = (corner(0, 1, 0), corner(0, 1, 1));
    let (c110, c111) = (corner(1, 1, 0), corner(1, 1, 1));

    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    let mut see = |v: f32| {
        lo = lo.min(v);
        hi = hi.max(v);
    };
    for dz in [0, cell.z - 1] {
        let alpha_z = dz as f32 * inv_xz;
        let v00 = lerp(alpha_z, c000, c001);
        let v10 = lerp(alpha_z, c100, c101);
        let v01 = lerp(alpha_z, c010, c011);
        let v11 = lerp(alpha_z, c110, c111);
        for dx in [0, cell.x - 1] {
            let alpha_x = dx as f32 * inv_xz;
            let bottom = lerp(alpha_x, v00, v10);
            let top = lerp(alpha_x, v01, v11);
            let step = (top - bottom) * inv_y;
            let mut value = mul_add(step, 0.0, bottom);
            see(value);
            for _ in 1..cell.y {
                value += step;
            }
            see(value);
        }
    }
    let slop = 2.0 * cell.y as f32 * f32::EPSILON * lo.abs().max(hi.abs());
    Interval::of(lo - slop, hi + slop)
}

/// One root's subgraph split at every `interpolated` node: the wrappers and
/// everything above them are bounded per cell, while their inputs are only ever
/// read off the cell lattice.
pub struct CellBounds {
    inputs: Box<[NodeId]>,
    wrappers: Box<[NodeId]>,
    /// Every reached node, ascending, wrappers included.
    terms: Box<[NodeId]>,
    /// Dense slot per node id; `NO_SLOT` for a node the walk never reached.
    slot: Box<[u32]>,
    root: NodeId,
    cell_size: Option<IVec3>,
    boundable: bool,
}

const NO_SLOT: u32 = u32::MAX;

impl CellBounds {
    pub(crate) fn new(program: &Program, root: NodeId) -> Self {
        let is_wrapper = |id: NodeId| matches!(program.node(id), Node::Interpolated { .. });

        let mut reached = vec![false; program.len()];
        let mut pending = vec![root];
        reached[root as usize] = true;
        while let Some(id) = pending.pop() {
            if is_wrapper(id) {
                continue;
            }
            program.node(id).visit_inputs(&mut |dep| {
                if !reached[dep as usize] {
                    reached[dep as usize] = true;
                    pending.push(dep);
                }
            });
        }

        let terms: Vec<NodeId> = (0..program.len() as NodeId)
            .filter(|&id| reached[id as usize])
            .collect();
        let wrappers: Vec<NodeId> = terms.iter().copied().filter(|&id| is_wrapper(id)).collect();
        let inputs: Vec<NodeId> = wrappers
            .iter()
            .map(|&id| match program.node(id) {
                Node::Interpolated { input, .. } => *input,
                _ => unreachable!("the wrapper list holds only interpolated nodes"),
            })
            .collect();

        let mut geometries = wrappers.iter().map(|&id| match program.node(id) {
            Node::Interpolated {
                cell_xz, cell_y, ..
            } => IVec3::new(*cell_xz, *cell_y, *cell_xz),
            _ => unreachable!("the wrapper list holds only interpolated nodes"),
        });
        let first = geometries.next();
        // Mixed geometries have no common lattice, so no whole-cell shortcut.
        let cell_size = geometries
            .all(|g| Some(g) == first)
            .then(|| first.unwrap_or(DEFAULT_CELL));

        let mut slot = vec![NO_SLOT; program.len()];
        for (index, &id) in terms.iter().enumerate() {
            slot[id as usize] = index as u32;
        }

        let mut bounds = Self {
            inputs: inputs.into_boxed_slice(),
            wrappers: wrappers.into_boxed_slice(),
            terms: terms.into_boxed_slice(),
            slot: slot.into_boxed_slice(),
            root,
            cell_size,
            boundable: true,
        };
        // Whether a node can be bounded depends on its kind and never on the
        // values, so one probe settles it for every cell.
        let probe = vec![Interval::exact(0.0); bounds.wrappers.len()];
        bounds.boundable = bounds
            .eval(program, &probe, IVec3::ZERO, IVec3::ZERO, &mut Vec::new())
            .is_some();
        bounds
    }

    /// The nodes to sample over the cell-corner lattice, in the order
    /// [`CellBounds::eval`] expects their corner intervals.
    #[inline]
    pub fn inputs(&self) -> &[NodeId] {
        &self.inputs
    }

    /// The `interpolated` nodes themselves, parallel to [`Self::inputs`].
    pub fn wrappers(&self) -> &[NodeId] {
        &self.wrappers
    }

    #[inline]
    pub fn cell_size(&self) -> Option<IVec3> {
        self.cell_size
    }

    /// Bounds on the root across a whole cell, given each `interpolated`
    /// wrapper's own bounds over the cell's eight corners and the cell's
    /// inclusive block box. Trilinear interpolation is a convex combination, so
    /// it never leaves the corner hull; interval arithmetic over the terms above
    /// carries that up.
    ///
    /// `None` when a term has a kind this cannot bound, which simply means the
    /// caller must evaluate the cell block by block.
    ///
    /// `values` is one bound per reached term, reused across the few hundred
    /// calls a column makes; a fresh vector each time would be the allocation,
    /// not the arithmetic.
    pub fn eval(
        &self,
        program: &Program,
        corners: &[Interval],
        min: IVec3,
        max: IVec3,
        values: &mut Vec<Interval>,
    ) -> Option<Interval> {
        if !self.boundable {
            return None;
        }
        assert_eq!(
            corners.len(),
            self.wrappers.len(),
            "one corner interval per interpolated wrapper"
        );
        values.clear();
        values.resize(self.terms.len(), Interval::NAI);
        for (k, &id) in self.wrappers.iter().enumerate() {
            values[self.slot[id as usize] as usize] = corners[k];
        }
        for &id in self.terms.iter() {
            let node = program.node(id);
            if matches!(node, Node::Interpolated { .. }) {
                continue;
            }
            let bound = node_bounds(node, Bounds::Cell { min, max }, |dep| {
                values[self.slot[dep as usize] as usize]
            })?;
            values[self.slot[id as usize] as usize] = bound;
        }
        Some(values[self.slot[self.root as usize] as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::gradient::{GradientParams, Tiling};
    use crate::program::{Node, Program, UnaryOp, Workspace};
    use crate::strata::{ALL_AXES, AXIS_X, AXIS_Y, Axes};
    use crate::volume::{Axis, Volume};

    /// A field that is not linear in any axis, so the interpolant it feeds has
    /// non-zero cross terms and the ramp is not the whole story.
    fn interpolated_program(cell: IVec3) -> Program {
        // The spans cover every sampled coordinate: a clamped gradient is
        // constant outside its own, and a constant field has nothing to refine.
        let gradient = |axis: Axis, from_value: f32, to_value: f32| {
            Node::Gradient(GradientParams {
                axis,
                tiling: Tiling::ClampToEdge,
                from: -128.0,
                to: 128.0,
                from_value,
                to_value,
            })
        };
        let nodes = vec![
            gradient(Axis::X, -3.0, 5.0),
            gradient(Axis::Y, -7.0, 7.0),
            Node::Binary {
                op: crate::program::BinaryOp::Mul,
                a: 0,
                b: 1,
            },
            Node::Unary {
                op: UnaryOp::Squeeze,
                input: 2,
            },
            Node::Interpolated {
                input: 3,
                cell_xz: cell.x,
                cell_y: cell.y,
                cell: 0,
            },
        ];
        let axes: Vec<Axes> = vec![AXIS_X, AXIS_Y, AXIS_X | AXIS_Y, AXIS_X | AXIS_Y, ALL_AXES];
        let ranges = vec![Interval::INFINITE; nodes.len()];
        Program::new(nodes, axes, ranges, vec![4])
    }

    /// `docs/worldgen.md` L5 for the tightened bound: it must contain every
    /// value the fill writes inside the cell. Both the density cells and the ore
    /// vein cells settle on this, and a bound that is a hair too narrow silently
    /// writes the wrong substance.
    #[test]
    fn a_sampled_range_contains_every_value_the_fill_writes() {
        const CELL: IVec3 = IVec3::new(4, 8, 4);
        let program = interpolated_program(CELL);
        let mut ws = Workspace::new();
        let (mut checked, mut tighter) = (0, 0);

        for base_z in [-64, -8, 0, 12, 40] {
            for base_x in [-32, -4, 0, 16, 48] {
                for base_y in [-64, -8, 0, 24, 56] {
                    let origin = IVec3::new(base_x, base_y, base_z);

                    // The eight corners, as the lattice fill hands them over.
                    let lattice = Volume::new(IVec3::splat(2), origin, CELL);
                    let mut corners = vec![0.0f32; lattice.len()];
                    program.fill(&mut ws, &lattice, 0, &mut corners);
                    let bound = sampled_range(CELL, |dx, dy, dz| {
                        corners[lattice.index_unchecked(dx, dy, dz)]
                    });

                    // Every block the fill writes inside that cell.
                    let blocks = Volume::dense(CELL, origin);
                    let mut values = vec![0.0f32; blocks.len()];
                    program.fill(&mut ws, &blocks, 0, &mut values);

                    for (at, &value) in values.iter().enumerate() {
                        assert!(
                            value >= bound.min() && value <= bound.max(),
                            "cell at {origin}: block {at} is {value:e}, outside [{:e}, {:e}]",
                            bound.min(),
                            bound.max()
                        );
                        checked += 1;
                    }

                    let hull = corners
                        .iter()
                        .fold(Interval::NAI, |acc, &v| acc.union_value(v));
                    if bound.max() - bound.min() < hull.max() - hull.min() {
                        tighter += 1;
                    }
                }
            }
        }
        assert_eq!(checked, 125 * 128);
        // The point of the refinement: the closed cell's far face is one lattice
        // step past anything the blocks read, so where the field has a gradient
        // the range they cover is narrower than the hull. The rest of the cells
        // sit where the squeeze saturates, and a constant field has no slack.
        assert!(
            tighter > 40,
            "only {tighter} of 125 cells came out tighter than the corner hull"
        );
    }
}
