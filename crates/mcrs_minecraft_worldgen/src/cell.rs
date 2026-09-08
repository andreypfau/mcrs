use crate::bounds::{Bounds, node_bounds};
use crate::interval::Interval;
use crate::program::{Node, NodeId, Program};
use bevy_math::IVec3;

/// The lattice a router with no `interpolated` node is still asked about.
const DEFAULT_CELL: IVec3 = IVec3::new(4, 8, 4);

/// Margin a whole-cell decision keeps away from zero. [`CellBounds::eval`] is
/// f32 interval arithmetic without outward rounding, so a bound that lands
/// exactly on zero is not trustworthy; cells inside the margin go block by block.
pub const CELL_BOUNDS_SLACK: f32 = 1e-5;

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
            .eval(program, &probe, IVec3::ZERO, IVec3::ZERO)
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
    pub fn eval(
        &self,
        program: &Program,
        corners: &[Interval],
        min: IVec3,
        max: IVec3,
    ) -> Option<Interval> {
        if !self.boundable {
            return None;
        }
        assert_eq!(
            corners.len(),
            self.wrappers.len(),
            "one corner interval per interpolated wrapper"
        );
        let mut values = vec![Interval::NAI; self.terms.len()];
        for (k, &id) in self.wrappers.iter().enumerate() {
            values[self.slot[id as usize] as usize] = corners[k];
        }
        for &id in self.terms.iter() {
            let node = program.node(id);
            if matches!(node, Node::Interpolated { .. }) {
                continue;
            }
            let bound = node_bounds(node, Bounds::Cell { min, max }, &|dep| {
                values[self.slot[dep as usize] as usize]
            })?;
            values[self.slot[id as usize] as usize] = bound;
        }
        Some(values[self.slot[self.root as usize] as usize])
    }
}
