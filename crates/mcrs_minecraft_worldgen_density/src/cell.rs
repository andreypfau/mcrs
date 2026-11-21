use crate::bounds::{Bounds, node_bounds};
use crate::program::{BinaryOp, Node, NodeId, Program, UnaryOp};
use glam::IVec3;
use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_noise::jmath::{lerp, mul_add};
use mcrs_minecraft_noise::sample_grid::SampleGrid;

const DEFAULT_CELL: IVec3 = IVec3::new(4, 8, 4);

pub const CELL_BOUNDS_SLACK: f32 = 1e-5;

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

pub fn corner_bounds(values: &[f32], lattice: &SampleGrid, at: IVec3, out: &mut [Interval]) {
    let stride = lattice.len();
    for (k, bound) in out.iter_mut().enumerate() {
        let row = values.get(k * stride..(k + 1) * stride).unwrap_or_default();
        *bound = corner_hull(row, lattice, at);
    }
}

fn corner_hull(row: &[f32], lattice: &SampleGrid, at: IVec3) -> Interval {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for dz in 0..2 {
        for dx in 0..2 {
            for dy in 0..2 {
                let index = lattice.index_unchecked(at.x + dx, at.y + dy, at.z + dz);
                let v = row.get(index).copied().unwrap_or_default();
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    interpolation_hull(Interval::of(lo, hi), lattice.step_block().y)
}

pub(crate) fn interpolation_hull(corners: Interval, cell_y: i32) -> Interval {
    if corners.is_nai() {
        return corners;
    }
    let magnitude = corners.min().abs().max(corners.max().abs());
    let slop = (cell_y as f32 + 20.0) * f32::EPSILON * magnitude;
    Interval::of(corners.min() - slop, corners.max() + slop)
}

pub fn sampled_bounds(
    values: &[f32],
    lattice: &SampleGrid,
    at: IVec3,
    cell: IVec3,
    out: &mut [Interval],
) {
    let stride = lattice.len();
    for (k, bound) in out.iter_mut().enumerate() {
        let row = values.get(k * stride..(k + 1) * stride).unwrap_or_default();
        *bound = sampled_range(cell, |dx, dy, dz| {
            let index = lattice.index_unchecked(at.x + dx, at.y + dy, at.z + dz);
            row.get(index).copied().unwrap_or_default()
        });
    }
}

struct ChainLink {
    node: NodeId,
    operand: NodeId,
    sibling: Interval,
}

struct UpperChain {
    wrapper: usize,
    links: Box<[ChainLink]>,
}

pub struct CellBounds {
    chain: Option<UpperChain>,
    inputs: Box<[NodeId]>,
    wrappers: Box<[NodeId]>,
    terms: Box<[NodeId]>,
    slot: Box<[u32]>,
    root: NodeId,
    cell_size: Option<IVec3>,
    boundable: bool,
}

const NO_SLOT: u32 = u32::MAX;

impl CellBounds {
    pub fn new(program: &Program, root: NodeId) -> Self {
        let is_wrapper = |id: NodeId| matches!(program.node(id), Node::Interpolated { .. });

        let mut reached = vec![false; program.len()];
        let mut pending = Vec::new();
        if let Some(reached) = reached.get_mut(root as usize) {
            *reached = true;
            pending.push(root);
        }
        while let Some(id) = pending.pop() {
            if is_wrapper(id) {
                continue;
            }
            program.node(id).visit_inputs(&mut |dep| {
                if let Some(reached @ false) = reached.get_mut(dep as usize) {
                    *reached = true;
                    pending.push(dep);
                }
            });
        }

        let terms: Vec<NodeId> = (0..program.len() as NodeId)
            .zip(&reached)
            .filter_map(|(id, &reached)| reached.then_some(id))
            .collect();
        let wrappers: Vec<NodeId> = terms.iter().copied().filter(|&id| is_wrapper(id)).collect();
        let inputs: Vec<NodeId> = wrappers
            .iter()
            .filter_map(|&id| match program.node(id) {
                Node::Interpolated { input, .. } => Some(*input),
                _ => None,
            })
            .collect();

        let mut geometries = wrappers.iter().filter_map(|&id| match program.node(id) {
            Node::Interpolated {
                cell_xz, cell_y, ..
            } => Some(IVec3::new(*cell_xz, *cell_y, *cell_xz)),
            _ => None,
        });
        let first = geometries.next();
        let cell_size = geometries
            .all(|g| Some(g) == first)
            .then(|| first.unwrap_or(DEFAULT_CELL));

        let mut slot = vec![NO_SLOT; program.len()];
        for (index, &id) in terms.iter().enumerate() {
            if let Some(slot) = slot.get_mut(id as usize) {
                *slot = index as u32;
            }
        }

        let mut bounds = Self {
            chain: upper_chain(program, root, &wrappers),
            inputs: inputs.into_boxed_slice(),
            wrappers: wrappers.into_boxed_slice(),
            terms: terms.into_boxed_slice(),
            slot: slot.into_boxed_slice(),
            root,
            cell_size,
            boundable: true,
        };
        let probe = vec![Interval::exact(0.0); bounds.wrappers.len()];
        bounds.boundable = bounds
            .eval(program, &probe, IVec3::ZERO, IVec3::ZERO, &mut Vec::new())
            .is_some();
        bounds
    }

    #[inline]
    pub fn inputs(&self) -> &[NodeId] {
        &self.inputs
    }

    pub fn wrappers(&self) -> &[NodeId] {
        &self.wrappers
    }

    #[inline]
    pub fn cell_size(&self) -> Option<IVec3> {
        self.cell_size
    }

    pub fn chain_bound(
        &self,
        program: &Program,
        values: &[f32],
        lattice: &SampleGrid,
        at: IVec3,
        min: IVec3,
        max: IVec3,
    ) -> Option<Interval> {
        let chain = self.chain.as_ref()?;
        let stride = lattice.len();
        let row = values.get(chain.wrapper * stride..(chain.wrapper + 1) * stride)?;
        self.chain_over(program, corner_hull(row, lattice, at), min, max)
    }

    pub(crate) fn chain_input(&self) -> Option<NodeId> {
        let chain = self.chain.as_ref()?;
        self.inputs.get(chain.wrapper).copied()
    }

    pub(crate) fn chain_over(
        &self,
        program: &Program,
        wrapper: Interval,
        min: IVec3,
        max: IVec3,
    ) -> Option<Interval> {
        let chain = self.chain.as_ref()?;
        let mut bound = wrapper;
        for link in &chain.links {
            bound = node_bounds(program.node(link.node), Bounds::Cell { min, max }, |dep| {
                if dep == link.operand {
                    bound
                } else {
                    link.sibling
                }
            })?;
        }
        Some(bound)
    }

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
        let slot = |id: NodeId| {
            self.slot
                .get(id as usize)
                .map_or(usize::MAX, |&s| s as usize)
        };
        for (&id, &corner) in self.wrappers.iter().zip(corners) {
            if let Some(value) = values.get_mut(slot(id)) {
                *value = corner;
            }
        }
        for &id in self.terms.iter() {
            let node = program.node(id);
            if matches!(node, Node::Interpolated { .. }) {
                continue;
            }
            let bound = node_bounds(node, Bounds::Cell { min, max }, |dep| {
                values.get(slot(dep)).copied().unwrap_or(Interval::NAI)
            })?;
            if let Some(value) = values.get_mut(slot(id)) {
                *value = bound;
            }
        }
        values.get(slot(self.root)).copied()
    }
}

fn upper_chain(program: &Program, root: NodeId, wrappers: &[NodeId]) -> Option<UpperChain> {
    let mut links = Vec::new();
    let mut id = root;
    for _ in 0..program.len() {
        if let Some(wrapper) = wrappers.iter().position(|&w| w == id) {
            links.reverse();
            return Some(UpperChain {
                wrapper,
                links: links.into_boxed_slice(),
            });
        }
        let link = chain_link(program, id)?;
        id = link.operand;
        links.push(link);
    }
    None
}

fn chain_link(program: &Program, node: NodeId) -> Option<ChainLink> {
    let constant = |id: NodeId| match program.node(id) {
        Node::Constant(value) if !value.is_nan() => Some(Interval::exact(*value)),
        _ => None,
    };
    let (operand, sibling) = match *program.node(node) {
        Node::Binary {
            op: BinaryOp::Min,
            a,
            ..
        } => (a, Interval::INFINITE),
        Node::Binary {
            op: BinaryOp::Add,
            a,
            b,
        } => match (constant(a), constant(b)) {
            (_, Some(sibling)) => (a, sibling),
            (Some(sibling), None) => (b, sibling),
            (None, None) => return None,
        },
        Node::Unary {
            op: UnaryOp::Squeeze,
            input,
        } => (input, Interval::NAI),
        Node::Affine { input, scale, .. } if scale > 0.0 && scale.is_finite() => {
            (input, Interval::NAI)
        }
        _ => return None,
    };
    Some(ChainLink {
        node,
        operand,
        sibling,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::gradient::{GradientParams, Tiling};
    use crate::program::{Node, Program, UnaryOp, Workspace};
    use mcrs_minecraft_noise::sample_grid::{Axis, SampleGrid};
    use mcrs_minecraft_noise::strata::{ALL_AXES, AXIS_X, AXIS_Y, Axes};

    fn interpolated_program(cell: IVec3) -> Program {
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

                    let lattice = SampleGrid::new(IVec3::splat(2), origin, CELL);
                    let mut corners = vec![0.0f32; lattice.len()];
                    program.fill(&mut ws, &lattice, 0, &mut corners);
                    let bound = sampled_range(CELL, |dx, dy, dz| {
                        corners[lattice.index_unchecked(dx, dy, dz)]
                    });

                    let blocks = SampleGrid::dense(CELL, origin);
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
        assert!(
            tighter > 40,
            "only {tighter} of 125 cells came out tighter than the corner hull"
        );
    }

    #[test]
    fn a_corner_hull_contains_every_value_the_fill_writes_at_a_large_magnitude() {
        const CELL: IVec3 = IVec3::new(4, 8, 4);
        let nodes = vec![
            Node::Gradient(GradientParams {
                axis: Axis::Y,
                tiling: Tiling::ClampToEdge,
                from: 0.0,
                to: 8.0,
                from_value: 16_777_238.0,
                to_value: 16_777_248.0,
            }),
            Node::Interpolated {
                input: 0,
                cell_xz: CELL.x,
                cell_y: CELL.y,
                cell: 0,
            },
        ];
        let program = Program::new(
            nodes,
            vec![AXIS_Y, ALL_AXES],
            vec![Interval::INFINITE; 2],
            vec![1],
        );
        let mut ws = Workspace::new();

        let lattice = SampleGrid::new(IVec3::splat(2), IVec3::ZERO, CELL);
        let mut corners = vec![0.0f32; lattice.len()];
        program.fill(&mut ws, &lattice, 0, &mut corners);
        let mut hull = [Interval::NAI];
        corner_bounds(&corners, &lattice, IVec3::ZERO, &mut hull);

        let blocks = SampleGrid::dense(CELL, IVec3::ZERO);
        let mut values = vec![0.0f32; blocks.len()];
        program.fill(&mut ws, &blocks, 0, &mut values);
        for (at, &value) in values.iter().enumerate() {
            assert!(
                hull[0].contains(value),
                "block {at} is {value}, outside the corner hull {:?} of {corners:?}",
                hull[0]
            );
        }
    }
}
