use crate::bounds::{Bounds, node_bounds};
use crate::cell::{CELL_BOUNDS_SLACK, CellBounds};
use crate::program::{Node, NodeId, Program, Seed, SeededPlan, Workspace};
use glam::IVec3;
use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_noise::strata::{AXIS_Y, NO_AXES};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Term {
    Unused,
    Fixed,
    Exact,
    Banded,
    Varying,
}

pub struct ColumnBound {
    target: NodeId,
    gather: Option<NodeId>,
    frontier: Box<[NodeId]>,
    seeded: Box<[SeededPlan]>,
    terms: Box<[Term]>,
    fixed: Box<[Interval]>,
}

impl ColumnBound {
    pub fn new(program: &mut Program, target: NodeId, inputs: &[NodeId]) -> Option<Self> {
        let len = program.len();
        let mut reached = vec![false; len];
        *reached.get_mut(target as usize)? = true;
        for id in (0..=target).rev() {
            if reached.get(id as usize) != Some(&true) {
                continue;
            }
            let node = program.node(id);
            if matches!(node, Node::Interpolated { .. } | Node::Slice { .. }) {
                return None;
            }
            node.visit_inputs(&mut |dep| {
                if let Some(reached) = reached.get_mut(dep as usize) {
                    *reached = true;
                }
            });
        }

        let mut terms = vec![Term::Unused; len];
        let mut fixed = vec![Interval::NAI; len];
        for id in 0..=target {
            if reached.get(id as usize) != Some(&true) {
                continue;
            }
            let node = program.node(id);
            let axes = program.axes_of(id);
            let term = if axes != NO_AXES && axes & AXIS_Y == 0 {
                Term::Exact
            } else {
                let mut term = if matches!(node, Node::Gradient(_)) {
                    Term::Banded
                } else {
                    Term::Fixed
                };
                node.visit_inputs(&mut |dep| match terms.get(dep as usize) {
                    Some(Term::Exact | Term::Varying) => term = Term::Varying,
                    Some(Term::Banded) if term == Term::Fixed => term = Term::Banded,
                    _ => {}
                });
                term
            };
            if term == Term::Fixed {
                let at = |dep: NodeId| fixed.get(dep as usize).copied().unwrap_or(Interval::NAI);
                let bound = term_bound(node, IVec3::ZERO, IVec3::ZERO, at)?;
                *fixed.get_mut(id as usize)? = bound;
            }
            *terms.get_mut(id as usize)? = term;
        }

        let mut needed = vec![false; len];
        if terms.get(target as usize) == Some(&Term::Exact) {
            *needed.get_mut(target as usize)? = true;
        }
        for (id, _) in (0..=target)
            .zip(&terms)
            .filter(|(_, term)| **term == Term::Varying)
        {
            program.node(id).visit_inputs(&mut |dep| {
                if let Some(needed) = needed.get_mut(dep as usize) {
                    *needed = true;
                }
            });
        }
        let frontier: Vec<NodeId> = (0..len as NodeId)
            .filter(|&id| {
                needed.get(id as usize) == Some(&true)
                    && terms.get(id as usize) == Some(&Term::Exact)
            })
            .collect();

        let (gather, seeded) = if frontier.is_empty() {
            (None, Vec::new())
        } else {
            let seeded: Option<Vec<SeededPlan>> = inputs
                .iter()
                .map(|&input| program.seeded_plan(input, &frontier))
                .collect();
            (Some(program.gather(&frontier)?), seeded?)
        };
        Some(Self {
            target,
            gather,
            frontier: frontier.into_boxed_slice(),
            seeded: seeded.into_boxed_slice(),
            terms: terms.into_boxed_slice(),
            fixed: fixed.into_boxed_slice(),
        })
    }

    pub fn first_air_row(
        &self,
        program: &Program,
        cells: &CellBounds,
        ws: &mut Workspace,
        lattice: &SampleGrid,
        hint: Option<i32>,
        scratch: &mut ColumnScratch,
    ) -> i32 {
        let size = lattice.size();
        let top = size.y - 1;
        let plane = SampleGrid::new(
            IVec3::new(size.x, 1, size.z),
            lattice.min_block(),
            lattice.step_block(),
        );
        let columns = plane.len();
        scratch.frontier.clear();
        scratch.frontier.resize(self.frontier.len() * columns, 0.0);
        scratch.plane = None;
        if let Some(gather) = self.gather {
            program.fill_gathered(ws, &plane, gather, &self.frontier, &mut scratch.frontier);
            scratch.plane = Some(plane);
        }
        scratch.values.clear();
        scratch.values.extend_from_slice(&self.fixed);
        scratch.stamps.clear();
        scratch.stamps.resize(self.fixed.len(), 0);
        scratch.columns.clear();
        scratch.columns.resize(columns, Interval::NAI);

        let mut is_air =
            |bottom: i32| self.band_is_air(program, cells, lattice, &plane, bottom, scratch);
        let (mut low, mut high) = (0, top);
        if let Some(hint) = hint.map(|hint| hint.clamp(0, top)) {
            let mut step = 1;
            if is_air(hint) {
                high = hint;
                while low < high {
                    let probe = high.saturating_sub(step).max(low);
                    if !is_air(probe) {
                        low = probe + 1;
                        break;
                    }
                    high = probe;
                    step = step.saturating_mul(2);
                }
            } else {
                low = (hint + 1).min(high);
                while low < high {
                    let probe = low.saturating_add(step - 1).min(high);
                    if is_air(probe) {
                        high = probe;
                        break;
                    }
                    low = probe + 1;
                    step = step.saturating_mul(2);
                }
            }
        }
        while low < high {
            let middle = low + (high - low) / 2;
            if is_air(middle) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }

    pub fn fill_inputs(
        &self,
        program: &Program,
        ws: &mut Workspace,
        volume: &SampleGrid,
        scratch: &ColumnScratch,
        out: &mut [f32],
    ) -> bool {
        let Some(plane) = scratch.plane.as_ref().filter(|_| !self.seeded.is_empty()) else {
            return false;
        };
        let seed = Seed {
            plane,
            values: &scratch.frontier,
        };
        let count = volume.len();
        out.len() == self.seeded.len() * count
            && self
                .seeded
                .iter()
                .zip(out.chunks_exact_mut(count))
                .all(|(&seeded, row)| program.fill_seeded(ws, volume, seeded, &seed, row))
    }

    fn band_is_air(
        &self,
        program: &Program,
        cells: &CellBounds,
        lattice: &SampleGrid,
        plane: &SampleGrid,
        bottom: i32,
        scratch: &mut ColumnScratch,
    ) -> bool {
        let size = lattice.size();
        let (low, high) = (lattice.block_y(bottom), lattice.block_y(size.y - 1));
        scratch.columns.fill(Interval::NAI);
        scratch.epoch = scratch.epoch.wrapping_add(1);
        scratch.band = scratch.epoch;
        for cell_z in 0..size.z - 1 {
            for cell_x in 0..size.x - 1 {
                let mut hull = Interval::NAI;
                for (dx, dz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let Some(column) = self.column(
                        program,
                        lattice,
                        plane,
                        cell_x + dx,
                        cell_z + dz,
                        low,
                        high,
                        scratch,
                    ) else {
                        return false;
                    };
                    hull = hull.union(column);
                }
                let min = IVec3::new(lattice.block_x(cell_x), low, lattice.block_z(cell_z));
                let max = IVec3::new(
                    lattice.block_x(cell_x + 1) - 1,
                    high - 1,
                    lattice.block_z(cell_z + 1) - 1,
                );
                if !cells
                    .chain_over(program, hull, min, max)
                    .is_some_and(|bound| bound.max() < -CELL_BOUNDS_SLACK)
                {
                    return false;
                }
            }
        }
        true
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one column of one band of the lattice"
    )]
    fn column(
        &self,
        program: &Program,
        lattice: &SampleGrid,
        plane: &SampleGrid,
        x: i32,
        z: i32,
        low: i32,
        high: i32,
        scratch: &mut ColumnScratch,
    ) -> Option<Interval> {
        let at = plane.index_unchecked(x, 0, z);
        let cached = scratch.columns.get(at).copied()?;
        if !cached.is_nai() {
            return Some(cached);
        }
        let count = plane.len();
        for (k, &id) in self.frontier.iter().enumerate() {
            let value = scratch.frontier.get(k * count + at).copied()?;
            if value.is_nan() {
                return None;
            }
            *scratch.values.get_mut(id as usize)? = Interval::exact(value);
        }
        let min = IVec3::new(lattice.block_x(x), low, lattice.block_z(z));
        let max = IVec3::new(min.x, high, min.z);
        let bound = self.eval(program, min, max, scratch)?;
        *scratch.columns.get_mut(at)? = bound;
        Some(bound)
    }

    fn eval(
        &self,
        program: &Program,
        min: IVec3,
        max: IVec3,
        scratch: &mut ColumnScratch,
    ) -> Option<Interval> {
        scratch.epoch = scratch.epoch.wrapping_add(1);
        let ColumnScratch {
            values,
            stamps,
            stack,
            epoch,
            band,
            ..
        } = scratch;
        let state = Evaluation {
            terms: &self.terms,
            stamps,
            epoch: *epoch,
            band: *band,
        };
        stack.clear();
        stack.push(self.target);
        while let Some(&id) = stack.last() {
            if state.done(id) {
                stack.pop();
                continue;
            }
            let node = program.node(id);
            if let Some(next) = state.pending(node, values) {
                stack.push(next);
                continue;
            }
            let at = |dep: NodeId| state.value(dep, values);
            let bound = term_bound(node, min, max, at)?;
            *values.get_mut(id as usize)? = bound;
            *state.stamps.get_mut(id as usize)? = state.epoch;
            stack.pop();
        }
        Some(state.value(self.target, values))
    }
}

struct Evaluation<'a> {
    terms: &'a [Term],
    stamps: &'a mut [u64],
    epoch: u64,
    band: u64,
}

impl Evaluation<'_> {
    fn done(&self, id: NodeId) -> bool {
        let since = match self.terms.get(id as usize) {
            Some(Term::Varying) => self.epoch,
            Some(Term::Banded) => self.band,
            _ => return true,
        };
        self.stamps
            .get(id as usize)
            .is_some_and(|&stamp| stamp >= since)
    }

    fn value(&self, id: NodeId, values: &[Interval]) -> Interval {
        if self.done(id) {
            values
                .get(id as usize)
                .copied()
                .unwrap_or(Interval::INFINITE)
        } else {
            Interval::INFINITE
        }
    }

    fn pending(&self, node: &Node, values: &[Interval]) -> Option<NodeId> {
        match *node {
            Node::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in,
                when_out,
            } => {
                if !self.done(input) {
                    return Some(input);
                }
                let input = self.value(input, values);
                let only_in = input.min() >= min_inclusive && input.max() < max_exclusive;
                let only_out = input.max() < min_inclusive || input.min() >= max_exclusive;
                [(when_in, !only_out), (when_out, !only_in)]
                    .into_iter()
                    .find(|&(arm, wanted)| wanted && !self.done(arm))
                    .map(|(arm, _)| arm)
            }
            Node::IntervalSelect {
                input,
                ref thresholds,
                ref arms,
            } => {
                if !self.done(input) {
                    return Some(input);
                }
                let input = self.value(input, values);
                let last = arms.len().saturating_sub(1);
                let position =
                    |bound: f32| thresholds.iter().position(|&t| bound < t).unwrap_or(last);
                arms.get(position(input.min())..=position(input.max()))
                    .unwrap_or_default()
                    .iter()
                    .copied()
                    .find(|&arm| !self.done(arm))
            }
            _ => {
                let mut next = None;
                node.visit_inputs(&mut |dep| {
                    if next.is_none() && !self.done(dep) {
                        next = Some(dep);
                    }
                });
                next
            }
        }
    }
}

#[derive(Default)]
pub struct ColumnScratch {
    frontier: Vec<f32>,
    values: Vec<Interval>,
    stamps: Vec<u64>,
    stack: Vec<NodeId>,
    columns: Vec<Interval>,
    plane: Option<SampleGrid>,
    epoch: u64,
    band: u64,
}

fn term_bound(
    node: &Node,
    min: IVec3,
    max: IVec3,
    at: impl Fn(NodeId) -> Interval,
) -> Option<Interval> {
    let bound = node_bounds(node, Bounds::Cell { min, max }, &at)
        .or_else(|| node_bounds(node, Bounds::Declared, &at))?;
    (!bound.is_nai()).then_some(bound)
}
