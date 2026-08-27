use super::{DensityFunctionComponent, DependentDensityFunction, Sampler};

pub(crate) const MAX_GUARD_DEPTH: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Step {
    Eval {
        start: u32,
        end: u32,
    },
    Guard {
        input: u32,
        min_inclusive: f32,
        max_exclusive: f32,
        want_in: bool,
        unguard: u32,
    },
    Unguard,
}

#[derive(Clone, Debug)]
pub(crate) struct BranchSchedule {
    pub order: Box<[usize]>,
    pub steps: Box<[Step]>,
    pub guarded: Box<[bool]>,
}

/// Walk backwards from `start` through input edges, returning a reachability bitmap
/// of size `extent`. Entries beyond `start` are always false.
pub(crate) fn reachable_backwards(
    start: usize,
    stack: &[DensityFunctionComponent],
    extent: usize,
) -> Vec<bool> {
    let mut visited = vec![false; extent];
    if start < extent {
        visited[start] = true;
    }
    for i in (0..=start.min(extent.saturating_sub(1))).rev() {
        if !visited[i] {
            continue;
        }
        stack[i].visit_input_indices(&mut |dep| {
            if dep < extent {
                visited[dep] = true;
            }
        });
    }
    visited
}

struct Cone {
    rc: usize,
    want_in: bool,
    nodes: Vec<usize>,
    member: Vec<bool>,
    parent: Option<usize>,
}

fn range_choice(stack: &[DensityFunctionComponent], i: usize) -> Option<&super::RangeChoice> {
    match &stack[i].sampler {
        Sampler::Dependent(DependentDensityFunction::RangeChoice(rc)) => Some(rc),
        _ => None,
    }
}

/// The nodes that only the `when_in` / only the `when_out` arm of `rc` can reach.
///
/// A node qualifies when every path from a root to it crosses that one arm edge,
/// so evaluating it is dead work whenever the selection lands on the other arm.
fn arm_cones(
    stack: &[DensityFunctionComponent],
    members: &[usize],
    extent: usize,
    roots: &[usize],
    rc_idx: usize,
) -> (Vec<usize>, Vec<usize>) {
    let rc = range_choice(stack, rc_idx).expect("arm_cones on a non-selection node");
    let from_in = reachable_backwards(rc.when_in_index, stack, extent);
    let from_out = reachable_backwards(rc.when_out_index, stack, extent);

    // Reachability runs over every index, not just the region's own members: a
    // member reached only by detouring through a node outside the region is
    // still reachable, and treating it as exclusive would drop a live value.
    let mut without = vec![false; extent];
    for &r in roots {
        if r < extent {
            without[r] = true;
        }
    }
    for i in (0..extent).rev() {
        if !without[i] {
            continue;
        }
        if i == rc_idx {
            without[rc.input_index] = true;
        } else {
            stack[i].visit_input_indices(&mut |dep| {
                if dep < extent {
                    without[dep] = true;
                }
            });
        }
    }

    let exclusive = |keep: &[bool], drop: &[bool]| -> Vec<usize> {
        members
            .iter()
            .copied()
            .filter(|&e| e < rc_idx && keep[e] && !drop[e] && !without[e])
            .collect()
    };
    (
        exclusive(&from_in, &from_out),
        exclusive(&from_out, &from_in),
    )
}

/// Order `members` so that every arm-exclusive cone occupies one contiguous run,
/// guarded by the selection that decides whether the run is needed at all.
///
/// `members` must be ascending and in topological order (inputs before users);
/// `roots` are the values the caller reads back out of the region.
pub(crate) fn build(
    stack: &[DensityFunctionComponent],
    members: &[usize],
    roots: &[usize],
) -> BranchSchedule {
    let extent = members.last().map_or(0, |&m| m + 1);
    let mut in_region = vec![false; extent];
    for &m in members {
        in_region[m] = true;
    }

    let mut cones: Vec<Cone> = Vec::new();
    for &rc_idx in members {
        let Some(rc) = range_choice(stack, rc_idx) else {
            continue;
        };
        // A selector produced outside the region is not guaranteed to have been
        // evaluated by the time this region runs, so those sites stay unguarded.
        if rc.input_index >= extent || !in_region[rc.input_index] {
            continue;
        }
        let (cone_in, cone_out) = arm_cones(stack, members, extent, roots, rc_idx);
        for (want_in, nodes) in [(true, cone_in), (false, cone_out)] {
            if nodes.is_empty() {
                continue;
            }
            let mut member = vec![false; extent];
            for &n in &nodes {
                member[n] = true;
            }
            cones.push(Cone {
                rc: rc_idx,
                want_in,
                nodes,
                member,
                parent: None,
            });
        }
    }

    // The cones are disjoint or nested by construction, but the argument runs
    // through edge dominance in the DAG rather than anything locally visible,
    // and a violation would silently drop live nodes out of the schedule.
    for a in 0..cones.len() {
        for b in (a + 1)..cones.len() {
            let (x, y) = (&cones[a], &cones[b]);
            let shared = x.nodes.iter().any(|&n| y.member[n]);
            if !shared {
                continue;
            }
            let x_in_y = x.nodes.iter().all(|&n| y.member[n]);
            let y_in_x = y.nodes.iter().all(|&n| x.member[n]);
            assert!(
                x_in_y || y_in_x,
                "selection cones overlap without nesting: rc {} arm {} vs rc {} arm {}",
                x.rc,
                x.want_in,
                y.rc,
                y.want_in
            );
        }
    }

    for c in 0..cones.len() {
        let mut parent = None;
        let mut best = usize::MAX;
        for p in 0..cones.len() {
            if p == c || cones[p].nodes.len() <= cones[c].nodes.len() {
                continue;
            }
            if cones[c].nodes.iter().all(|&n| cones[p].member[n]) && cones[p].nodes.len() < best {
                best = cones[p].nodes.len();
                parent = Some(p);
            }
        }
        cones[c].parent = parent;
    }

    let mut owner = vec![usize::MAX; extent];
    for (ci, c) in cones.iter().enumerate() {
        for &n in &c.nodes {
            if owner[n] == usize::MAX || cones[owner[n]].nodes.len() > c.nodes.len() {
                owner[n] = ci;
            }
        }
    }

    let mut order: Vec<usize> = Vec::with_capacity(members.len());
    let mut steps: Vec<Step> = Vec::new();
    let mut run_start = 0u32;
    let mut guard_at: Vec<usize> = Vec::new();

    struct Emit<'a> {
        stack: &'a [DensityFunctionComponent],
        cones: &'a [Cone],
        owner: &'a [usize],
        order: &'a mut Vec<usize>,
        steps: &'a mut Vec<Step>,
        run_start: &'a mut u32,
        guard_at: &'a mut Vec<usize>,
        depth: usize,
    }

    impl Emit<'_> {
        fn flush(&mut self) {
            let end = self.order.len() as u32;
            if end > *self.run_start {
                self.steps.push(Step::Eval {
                    start: *self.run_start,
                    end,
                });
                *self.run_start = end;
            }
        }

        fn region(&mut self, nodes: &[usize], region: Option<usize>) {
            assert!(
                self.depth < MAX_GUARD_DEPTH,
                "selection nesting deeper than {MAX_GUARD_DEPTH}"
            );
            for &i in nodes {
                let direct = match self.owner[i] {
                    usize::MAX => region.is_none(),
                    o => Some(o) == region,
                };
                if !direct {
                    continue;
                }
                for ci in 0..self.cones.len() {
                    let c = &self.cones[ci];
                    if c.rc != i {
                        continue;
                    }
                    assert_eq!(
                        c.parent, region,
                        "cone of rc {i} is not nested where its selection node sits"
                    );
                    let rc = range_choice(self.stack, i).expect("cone without a selection node");
                    self.flush();
                    let guard = self.steps.len();
                    self.steps.push(Step::Guard {
                        input: rc.input_index as u32,
                        min_inclusive: rc.min_inclusion_value,
                        max_exclusive: rc.max_exclusion_value,
                        want_in: c.want_in,
                        unguard: 0,
                    });
                    self.guard_at.push(self.order.len());
                    let cone_nodes = c.nodes.clone();
                    self.depth += 1;
                    self.region(&cone_nodes, Some(ci));
                    self.depth -= 1;
                    self.flush();
                    let unguard = self.steps.len() as u32;
                    self.steps.push(Step::Unguard);
                    match &mut self.steps[guard] {
                        Step::Guard { unguard: u, .. } => *u = unguard,
                        _ => unreachable!(),
                    }
                }
                self.order.push(i);
            }
        }
    }

    let mut emit = Emit {
        stack,
        cones: &cones,
        owner: &owner,
        order: &mut order,
        steps: &mut steps,
        run_start: &mut run_start,
        guard_at: &mut guard_at,
        depth: 0,
    };
    emit.region(members, None);
    emit.flush();

    assert_eq!(
        order.len(),
        members.len(),
        "schedule dropped or duplicated region nodes"
    );

    let mut position = vec![usize::MAX; extent];
    for (p, &n) in order.iter().enumerate() {
        assert_eq!(position[n], usize::MAX, "node {n} scheduled twice");
        position[n] = p;
    }
    for (p, &n) in order.iter().enumerate() {
        let mut ok = true;
        stack[n].visit_input_indices(&mut |dep| {
            if dep < extent && in_region[dep] && position[dep] >= p {
                ok = false;
            }
        });
        assert!(ok, "schedule breaks topological order at node {n}");
    }

    let mut gi = 0usize;
    for step in &steps {
        if let Step::Guard { input, .. } = step {
            let emitted = guard_at[gi];
            gi += 1;
            let input = *input as usize;
            assert!(
                !in_region[input] || position[input] < emitted,
                "guard reads node {input} before it is evaluated"
            );
        }
    }

    let mut guarded = vec![false; extent];
    for c in &cones {
        for &n in &c.nodes {
            guarded[n] = true;
        }
    }

    BranchSchedule {
        order: order.into_boxed_slice(),
        steps: steps.into_boxed_slice(),
        guarded: guarded.into_boxed_slice(),
    }
}
