use crate::program::{BinaryOp, Node, NodeId};
use core::ops::Range;
use mcrs_minecraft_noise::interval::Interval;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum GuardTest {
    InRange {
        selector: NodeId,
        lo: f32,
        hi: f32,
        want: bool,
    },
    Arm {
        site: NodeId,
        selector: NodeId,
        arm: u32,
    },
    LeftAbove {
        left: NodeId,
        limit: f32,
    },
    LeftBelow {
        left: NodeId,
        limit: f32,
    },
    LeftNonZero {
        left: NodeId,
    },
}

impl GuardTest {
    pub(crate) fn selector(self) -> NodeId {
        match self {
            GuardTest::InRange { selector, .. } | GuardTest::Arm { selector, .. } => selector,
            GuardTest::LeftAbove { left, .. }
            | GuardTest::LeftBelow { left, .. }
            | GuardTest::LeftNonZero { left } => left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fallback {
    pub site: NodeId,
    pub source: NodeId,
    pub negate: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Step {
    Eval {
        end: u32,
    },
    Guard {
        test: GuardTest,
        fallback: Option<Fallback>,
        scope: Scope,
        skip_to: u32,
        next_step: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Volume,
    Elements,
}

struct Arm {
    target: NodeId,
    test: GuardTest,
    fallback: Option<Fallback>,
}

struct Site {
    always: Vec<NodeId>,
    arms: Vec<Arm>,
}

struct Cone {
    site: NodeId,
    test: GuardTest,
    fallback: Option<Fallback>,
    nodes: Vec<NodeId>,
    member: Vec<bool>,
}

pub(crate) fn build(
    nodes: &[Node],
    ranges: &[Interval],
    target: NodeId,
    column_order: &[NodeId],
) -> (Vec<NodeId>, Vec<Step>) {
    let mut cones: Vec<Cone> = Vec::new();
    for &site in column_order {
        let Some(descriptor) = nodes
            .get(site as usize)
            .and_then(|node| site_of(node, ranges, site))
        else {
            continue;
        };
        let outside = reachable_past_arms(nodes, target, site, &descriptor.always);
        let arms: Vec<Vec<bool>> = descriptor
            .arms
            .iter()
            .map(|arm| reachable(nodes, arm.target))
            .collect();
        for (k, (arm, reach)) in descriptor.arms.iter().zip(&arms).enumerate() {
            let members: Vec<NodeId> = column_order
                .iter()
                .copied()
                .filter(|&e| {
                    is(reach, e)
                        && !is(&outside, e)
                        && arms.iter().enumerate().all(|(j, r)| j == k || !is(r, e))
                })
                .collect();
            if members.is_empty() {
                continue;
            }
            let mut member = vec![false; nodes.len()];
            for &n in &members {
                mark(&mut member, n);
            }
            cones.push(Cone {
                site,
                test: arm.test,
                fallback: arm.fallback,
                nodes: members,
                member,
            });
        }
    }

    for (a, x) in cones.iter().enumerate() {
        for y in cones.iter().skip(a + 1) {
            if !x.nodes.iter().any(|&n| is(&y.member, n)) {
                continue;
            }
            assert!(
                x.nodes.iter().all(|&n| is(&y.member, n))
                    || y.nodes.iter().all(|&n| is(&x.member, n)),
                "guard cones overlap without nesting at nodes {} and {}",
                x.site,
                y.site
            );
        }
    }

    fn smallest<'a>(candidates: impl Iterator<Item = (usize, &'a Cone)>) -> Option<usize> {
        candidates
            .min_by_key(|(_, cone)| cone.nodes.len())
            .map(|(c, _)| c)
    }
    let parents: Vec<Option<usize>> = cones
        .iter()
        .enumerate()
        .map(|(c, cone)| {
            smallest(cones.iter().enumerate().filter(|&(p, parent)| {
                p != c
                    && parent.nodes.len() > cone.nodes.len()
                    && cone.nodes.iter().all(|&n| is(&parent.member, n))
            }))
        })
        .collect();
    let mut owner: Vec<Option<usize>> = vec![None; nodes.len()];
    for &n in column_order {
        if let Some(owner) = owner.get_mut(n as usize) {
            *owner = smallest(
                cones
                    .iter()
                    .enumerate()
                    .filter(|(_, cone)| is(&cone.member, n)),
            );
        }
    }

    let mut emit = Emit {
        nodes,
        cones: &cones,
        parents: &parents,
        owner: &owner,
        order: Vec::with_capacity(column_order.len()),
        steps: Vec::new(),
        run_start: 0,
        reads: Vec::new(),
    };
    emit.region(column_order, None);
    emit.flush();
    verify(nodes, column_order, &emit.order, &emit.reads);
    (emit.order, emit.steps)
}

struct Emit<'a> {
    nodes: &'a [Node],
    cones: &'a [Cone],
    parents: &'a [Option<usize>],
    owner: &'a [Option<usize>],
    order: Vec<NodeId>,
    steps: Vec<Step>,
    run_start: u32,
    reads: Vec<(NodeId, usize)>,
}

impl Emit<'_> {
    fn flush(&mut self) {
        let end = self.order.len() as u32;
        if end > self.run_start {
            self.steps.push(Step::Eval { end });
            self.run_start = end;
        }
    }

    fn region(&mut self, members: &[NodeId], region: Option<usize>) {
        let (cones, parents) = (self.cones, self.parents);
        for &site in members {
            if self.owner.get(site as usize).copied().flatten() != region {
                continue;
            }
            let mut covered = false;
            for (c, (cone, &parent)) in cones.iter().zip(parents).enumerate() {
                if cone.site != site || parent != region {
                    continue;
                }
                let (test, fallback) = (cone.test, cone.fallback);
                self.flush();
                let guard = self.steps.len();
                self.steps.push(Step::Guard {
                    test,
                    fallback,
                    scope: Scope::Volume,
                    skip_to: 0,
                    next_step: 0,
                });
                let start = self.order.len();
                self.reads.push((test.selector(), start));
                self.region(&cone.nodes, Some(c));
                if fallback.is_some() {
                    self.order.push(site);
                    covered = true;
                }
                self.flush();
                let (end, next) = (self.order.len() as u32, self.steps.len() as u32);
                let narrows = self
                    .order
                    .get(start..)
                    .unwrap_or_default()
                    .iter()
                    .any(|&n| {
                        self.nodes
                            .get(n as usize)
                            .is_some_and(Node::samples_per_element)
                    });
                if let Some(Step::Guard {
                    scope,
                    skip_to,
                    next_step,
                    ..
                }) = self.steps.get_mut(guard)
                {
                    (*skip_to, *next_step) = (end, next);
                    if narrows {
                        *scope = Scope::Elements;
                    }
                }
            }
            if !covered {
                self.order.push(site);
            }
        }
    }
}

fn verify(nodes: &[Node], column_order: &[NodeId], order: &[NodeId], reads: &[(NodeId, usize)]) {
    let mut position = vec![usize::MAX; nodes.len()];
    for (p, &n) in order.iter().enumerate() {
        if let Some(at) = position.get_mut(n as usize) {
            assert_eq!(*at, usize::MAX, "node {n} scheduled twice");
            *at = p;
        }
    }
    let position_of = |id: NodeId| position.get(id as usize).copied().unwrap_or(usize::MAX);
    assert_eq!(
        order.len(),
        column_order.len(),
        "the schedule dropped nodes from the column order"
    );
    for (p, &n) in order.iter().enumerate() {
        let Some(node) = nodes.get(n as usize) else {
            continue;
        };
        node.visit_local_inputs(&mut |j| {
            let at = position_of(j);
            assert!(
                at == usize::MAX || at < p,
                "the schedule evaluates node {n} before its input {j}"
            );
        });
    }
    for &(selector, at) in reads {
        let p = position_of(selector);
        assert!(
            p == usize::MAX || p < at,
            "a guard reads node {selector} before it is evaluated"
        );
    }
}

fn site_of(node: &Node, ranges: &[Interval], site: NodeId) -> Option<Site> {
    let plain = |target: NodeId, test: GuardTest| Arm {
        target,
        test,
        fallback: None,
    };
    match node {
        Node::RangeChoice {
            input,
            min_inclusive,
            max_exclusive,
            when_in,
            when_out,
        } => {
            let test = |want| GuardTest::InRange {
                selector: *input,
                lo: *min_inclusive,
                hi: *max_exclusive,
                want,
            };
            Some(Site {
                always: vec![*input],
                arms: vec![plain(*when_in, test(true)), plain(*when_out, test(false))],
            })
        }
        Node::IntervalSelect { input, arms, .. } => Some(Site {
            always: vec![*input],
            arms: arms
                .iter()
                .enumerate()
                .map(|(k, &target)| {
                    let test = GuardTest::Arm {
                        site,
                        selector: *input,
                        arm: k as u32,
                    };
                    plain(target, test)
                })
                .collect(),
        }),
        Node::Lerp {
            alpha,
            first,
            second,
        } => Some(Site {
            always: vec![*alpha, *first],
            arms: vec![plain(*second, GuardTest::LeftNonZero { left: *alpha })],
        }),
        Node::Binary { op, a, b } => {
            let (left, right) = (*a, ranges.get(*b as usize).copied()?);
            let (test, negate) = match op {
                BinaryOp::Min if right.min().is_finite() => (
                    GuardTest::LeftAbove {
                        left,
                        limit: right.min(),
                    },
                    false,
                ),
                BinaryOp::Max if right.max().is_finite() => (
                    GuardTest::LeftBelow {
                        left,
                        limit: right.max(),
                    },
                    false,
                ),
                BinaryOp::Mul if right.min() > 0.0 && right.max().is_finite() => {
                    (GuardTest::LeftNonZero { left }, false)
                }
                BinaryOp::Mul if right.max() < 0.0 && right.min().is_finite() => {
                    (GuardTest::LeftNonZero { left }, true)
                }
                _ => return None,
            };
            let fallback = Some(Fallback {
                site,
                source: left,
                negate,
            });
            Some(Site {
                always: vec![left],
                arms: vec![Arm {
                    target: *b,
                    test,
                    fallback,
                }],
            })
        }
        _ => None,
    }
}

fn is(set: &[bool], id: NodeId) -> bool {
    set.get(id as usize).copied().unwrap_or(false)
}

fn mark(set: &mut [bool], id: NodeId) {
    if let Some(member) = set.get_mut(id as usize) {
        *member = true;
    }
}

fn walk_down(nodes: &[Node], last: NodeId) -> impl Iterator<Item = (usize, &Node)> {
    nodes
        .get(..=last as usize)
        .unwrap_or_default()
        .iter()
        .enumerate()
        .rev()
}

fn reachable(nodes: &[Node], start: NodeId) -> Vec<bool> {
    let mut seen = vec![false; nodes.len()];
    mark(&mut seen, start);
    for (i, node) in walk_down(nodes, start) {
        if is(&seen, i as NodeId) {
            node.visit_local_inputs(&mut |j| mark(&mut seen, j));
        }
    }
    seen
}

fn reachable_past_arms(
    nodes: &[Node],
    target: NodeId,
    site: NodeId,
    always: &[NodeId],
) -> Vec<bool> {
    let mut seen = vec![false; nodes.len()];
    mark(&mut seen, target);
    for (i, node) in walk_down(nodes, target) {
        if !is(&seen, i as NodeId) {
            continue;
        }
        if i as NodeId == site {
            for &j in always {
                mark(&mut seen, j);
            }
        } else {
            node.visit_local_inputs(&mut |j| mark(&mut seen, j));
        }
    }
    seen
}

#[derive(Clone, Copy)]
pub(crate) enum Keep<'a> {
    Always,
    InRange {
        lo: f32,
        hi: f32,
        want: bool,
    },
    Arm {
        thresholds: &'a [f32],
        last: usize,
        arm: usize,
    },
    Above(f32),
    Below(f32),
    NonZero,
}

impl<'a> Keep<'a> {
    pub(crate) fn new(test: GuardTest, node_of: impl Fn(NodeId) -> &'a Node) -> Self {
        match test {
            GuardTest::InRange { lo, hi, want, .. } => Keep::InRange { lo, hi, want },
            GuardTest::Arm { site, arm, .. } => match node_of(site) {
                Node::IntervalSelect {
                    thresholds, arms, ..
                } => Keep::Arm {
                    thresholds,
                    last: arms.len().saturating_sub(1),
                    arm: arm as usize,
                },
                _ => Keep::Always,
            },
            GuardTest::LeftAbove { limit, .. } => Keep::Above(limit),
            GuardTest::LeftBelow { limit, .. } => Keep::Below(limit),
            GuardTest::LeftNonZero { .. } => Keep::NonZero,
        }
    }

    pub(crate) fn coverage(self, values: &[f32], scope: Scope) -> Coverage {
        match self {
            Keep::Always => Coverage::Everything,
            Keep::InRange { lo, hi, want } => {
                coverage_where(values, scope, |v| (v >= lo && v < hi) == want)
            }
            Keep::Arm {
                thresholds,
                last,
                arm,
            } => coverage_where(values, scope, |v| {
                thresholds.iter().position(|&t| v < t).unwrap_or(last) == arm
            }),
            Keep::Above(limit) => coverage_where(values, scope, |v| v > limit),
            Keep::Below(limit) => coverage_where(values, scope, |v| v < limit),
            Keep::NonZero => coverage_where(values, scope, |v| v != 0.0),
        }
    }

    pub(crate) fn kept_runs(
        self,
        column: &[f32],
        rows: Range<usize>,
        emit: impl FnMut(Range<usize>),
    ) {
        match self {
            Keep::Always => runs_where(column, rows, emit, |_| true),
            Keep::InRange { lo, hi, want } => {
                runs_where(column, rows, emit, |v| (v >= lo && v < hi) == want)
            }
            Keep::Arm {
                thresholds,
                last,
                arm,
            } => runs_where(column, rows, emit, |v| {
                thresholds.iter().position(|&t| v < t).unwrap_or(last) == arm
            }),
            Keep::Above(limit) => runs_where(column, rows, emit, |v| v > limit),
            Keep::Below(limit) => runs_where(column, rows, emit, |v| v < limit),
            Keep::NonZero => runs_where(column, rows, emit, |v| v != 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Coverage {
    Nothing,
    Part,
    Everything,
}

impl Coverage {
    #[must_use]
    pub(crate) fn and(self, other: Coverage) -> Coverage {
        if self == other { self } else { Coverage::Part }
    }
}

#[inline(always)]
fn coverage_where(values: &[f32], scope: Scope, keeps: impl Fn(f32) -> bool) -> Coverage {
    let Some((&first, rest)) = values.split_first() else {
        return Coverage::Nothing;
    };
    let first = keeps(first);
    let mixed = match scope {
        Scope::Volume => !first && rest.iter().any(|&value| keeps(value)),
        Scope::Elements => rest.iter().any(|&value| keeps(value) != first),
    };
    match (first, mixed, scope) {
        (_, true, Scope::Volume) | (true, false, _) => Coverage::Everything,
        (_, true, Scope::Elements) => Coverage::Part,
        (false, false, _) => Coverage::Nothing,
    }
}

#[inline(always)]
fn runs_where(
    column: &[f32],
    rows: Range<usize>,
    mut emit: impl FnMut(Range<usize>),
    keeps: impl Fn(f32) -> bool,
) {
    if let [value] = column {
        if keeps(*value) {
            emit(rows);
        }
        return;
    }
    let Some(values) = column.get(rows.clone()) else {
        return;
    };
    let mut start = None;
    for (y, &value) in (rows.start..).zip(values) {
        match (keeps(value), start) {
            (true, None) => start = Some(y),
            (false, Some(from)) => {
                emit(from..y);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        emit(from..rows.end);
    }
}
