use core::fmt;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::placement_filter::{
    FaceShape, PlacementCondition, PlacementRule, StateMatch, SupportPredicate, ValueMatch,
};
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockEntry, BlockStateFlags};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::direction::DirectionSet;
use mcrs_minecraft_registries::{Registries, TagEntry, Tags};
use mcrs_minecraft_resource::{Id, ResourceLocationBuf, TagKey};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::face_test;

/// The support of each block state, baked from the block definitions and the tags of a data pack.
#[derive(Clone, Debug, Default)]
pub struct BlockSupport {
    subjects: Box<[Option<Subject>]>,
    programs: Vec<Program>,
    sets: Vec<FixedBitSet>,
}

#[derive(Clone, Copy, Debug)]
struct Subject {
    program: u16,
    part: Part,
    block: Option<Id<Block>>,
}

/// The part of a tall plant (SUP-VEG-002) that a state is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Whole,
    Lower,
    Upper,
}

#[derive(Clone, Debug)]
struct Program {
    node: Node,
    tested: DirectionSet,
}

#[derive(Clone, Debug)]
enum Node {
    True,
    Leaf { at: Option<Direction>, set: usize },
    Not(Box<Node>),
    AnyOf(Box<[Node]>),
    AllOf(Box<[Node]>),
}

/// A name that a support predicate reads and the loaded data pack does not hold. The leaf that
/// reads it matches no state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportIssue {
    pub block: ResourceLocationBuf,
    pub missing: Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    Block(ResourceLocationBuf),
    BlockTag(ResourceLocationBuf),
    Fluid(ResourceLocationBuf),
}

impl fmt::Display for SupportIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (what, name) = match &self.missing {
            Missing::Block(name) => ("block", name),
            Missing::BlockTag(name) => ("block tag", name),
            Missing::Fluid(name) => ("fluid", name),
        };
        write!(
            f,
            "the support predicate of {} reads the unknown {what} {name}",
            self.block
        )
    }
}

impl BlockSupport {
    /// Bakes the support predicate of each state of `definitions` against the registries and the
    /// block tags of `registries`.
    pub fn bake(
        definitions: &BlockDefinitions,
        registries: &Registries,
    ) -> (Self, Vec<SupportIssue>) {
        let tags = registries.tags::<Block>();
        let mut baker = Baker {
            definitions,
            registries,
            tags: tags.as_ref(),
            sets: Vec::new(),
            issues: Vec::new(),
            block: None,
        };
        let mut programs: Vec<(&PlacementRule, Option<DirectionSet>, Program)> = Vec::new();
        let mut subjects = vec![None; definitions.state_count()].into_boxed_slice();
        for entry in definitions.blocks() {
            baker.block = Some(entry.identifier.clone());
            for offset in 0..entry.state_count {
                let Some(state) = entry.state_at(offset) else {
                    continue;
                };
                // The upper part of a tall plant needs no filter of its own: its lower part has one.
                let lower = || {
                    (part_of(entry, state) == Part::Upper)
                        .then(|| entry.with_text(state, "half", "lower"))
                        .flatten()
                };
                let filter = definitions
                    .placement_filter(state)
                    .or_else(|| lower().and_then(|lower| definitions.placement_filter(lower)));
                let Some(filter) = filter else {
                    continue;
                };
                let known = programs.iter().position(|(rule, tested, _)| {
                    *rule == &filter.rule && *tested == filter.tested_directions
                });
                let program = match known {
                    Some(index) => index,
                    None => {
                        let node = baker.rule(&filter.rule);
                        let tested = filter
                            .tested_directions
                            .unwrap_or_else(|| directions_of(&node));
                        programs.push((
                            &filter.rule,
                            filter.tested_directions,
                            Program { node, tested },
                        ));
                        programs.len() - 1
                    }
                };
                if let (Ok(program), Some(slot)) =
                    (u16::try_from(program), subjects.get_mut(state.index()))
                {
                    *slot = Some(Subject {
                        program,
                        part: part_of(entry, state),
                        block: definitions.block_index(state),
                    });
                }
            }
        }
        let support = BlockSupport {
            subjects,
            programs: programs
                .into_iter()
                .map(|(_, _, program)| program)
                .collect(),
            sets: baker.sets,
        };
        (support, baker.issues)
    }

    /// Whether a block in `state` at `at` can stay among the states that `read` gives
    /// (SUP-PRED-001, SUP-VEG-002). A state without a support predicate can always stay.
    pub fn can_stay(
        &self,
        state: BlockStateId,
        at: BlockPos,
        mut read: impl FnMut(BlockPos) -> BlockStateId,
    ) -> bool {
        let Some(subject) = self.subject(state) else {
            return true;
        };
        match subject.part {
            Part::Upper => self.holds(subject, read(at + Direction::Down.normal()), Part::Lower),
            Part::Whole | Part::Lower => self.eval(subject, at, &mut read),
        }
    }

    /// The shape function of a subject (SUP-RUN-001): whether it stays after a shape update that
    /// comes from `from`. A subject that does not stay is removed.
    pub fn stays_after_update(
        &self,
        state: BlockStateId,
        at: BlockPos,
        from: Direction,
        mut read: impl FnMut(BlockPos) -> BlockStateId,
    ) -> bool {
        let Some(subject) = self.subject(state) else {
            return true;
        };
        match (subject.part, from) {
            (Part::Whole, _) => {
                let tested = self
                    .programs
                    .get(usize::from(subject.program))
                    .is_some_and(|program| program.tested.contains(from));
                !tested || self.eval(subject, at, &mut read)
            }
            (Part::Lower, Direction::Down) => self.eval(subject, at, &mut read),
            (Part::Lower, Direction::Up) => {
                self.holds(subject, read(at + Direction::Up.normal()), Part::Upper)
            }
            (Part::Upper, Direction::Down) => {
                self.holds(subject, read(at + Direction::Down.normal()), Part::Lower)
            }
            _ => true,
        }
    }

    /// Whether `state` is one part of a block of two parts.
    pub fn is_two_part(&self, state: BlockStateId) -> bool {
        self.subject(state)
            .is_some_and(|subject| subject.part != Part::Whole)
    }

    fn subject(&self, state: BlockStateId) -> Option<Subject> {
        self.subjects.get(state.index()).copied().flatten()
    }

    fn holds(&self, subject: Subject, other: BlockStateId, part: Part) -> bool {
        self.subject(other)
            .is_some_and(|other| other.part == part && other.block == subject.block)
    }

    fn eval(
        &self,
        subject: Subject,
        at: BlockPos,
        read: &mut impl FnMut(BlockPos) -> BlockStateId,
    ) -> bool {
        self.programs
            .get(usize::from(subject.program))
            .is_some_and(|program| self.node(&program.node, at, read))
    }

    fn node(
        &self,
        node: &Node,
        at: BlockPos,
        read: &mut impl FnMut(BlockPos) -> BlockStateId,
    ) -> bool {
        match node {
            Node::True => true,
            Node::Leaf { at: offset, set } => {
                let pos = offset.map_or(at, |direction| at + direction.normal());
                let state = read(pos);
                self.sets
                    .get(*set)
                    .is_some_and(|set| set.contains(state.index()))
            }
            Node::Not(node) => !self.node(node, at, read),
            Node::AnyOf(nodes) => nodes.iter().any(|node| self.node(node, at, read)),
            Node::AllOf(nodes) => nodes.iter().all(|node| self.node(node, at, read)),
        }
    }
}

fn part_of(entry: &BlockEntry, state: BlockStateId) -> Part {
    match entry.value_of(state, "half") {
        Some(value) if value.renders_to("lower") => Part::Lower,
        Some(value) if value.renders_to("upper") => Part::Upper,
        _ => Part::Whole,
    }
}

/// The default tested directions (SUP-PRED-003): the directions of the offsets of the leaves.
fn directions_of(node: &Node) -> DirectionSet {
    let mut set = DirectionSet::default();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        match node {
            Node::True => {}
            Node::Leaf { at, .. } => {
                if let Some(direction) = at {
                    set.insert(*direction);
                }
            }
            Node::Not(node) => stack.push(node),
            Node::AnyOf(nodes) | Node::AllOf(nodes) => stack.extend(nodes.iter()),
        }
    }
    set
}

struct Baker<'a> {
    definitions: &'a BlockDefinitions,
    registries: &'a Registries,
    tags: Option<&'a Tags<Block>>,
    sets: Vec<FixedBitSet>,
    issues: Vec<SupportIssue>,
    block: Option<ResourceLocationBuf>,
}

impl Baker<'_> {
    /// The short form becomes a predicate as SUP-DATA-002 states.
    fn rule(&mut self, rule: &PlacementRule) -> Node {
        match rule {
            PlacementRule::Predicate(predicate) => self.predicate(predicate),
            PlacementRule::Conditions(conditions) => Node::AnyOf(
                conditions
                    .iter()
                    .map(|condition| self.condition(condition))
                    .collect(),
            ),
        }
    }

    fn condition(&mut self, condition: &PlacementCondition) -> Node {
        let mut leaves = Vec::new();
        for face in &condition.allowed_faces {
            for entry in &condition.block_filter {
                let set = self.blocks(core::slice::from_ref(entry), &StateMatch::default());
                leaves.push(Node::Leaf {
                    at: Some(face.opposite()),
                    set,
                });
            }
        }
        Node::AnyOf(leaves.into_boxed_slice())
    }

    fn predicate(&mut self, predicate: &SupportPredicate) -> Node {
        let leaf = |at: &mcrs_minecraft_block::definition::placement_filter::Offset, set| {
            Node::Leaf { at: at.0, set }
        };
        match predicate {
            SupportPredicate::MatchingBlocks {
                offset,
                blocks,
                state,
            } => leaf(offset, self.blocks(blocks, state)),
            SupportPredicate::MatchingBlockTag { offset, tag } => {
                let entry = TagEntry {
                    id: tag.clone(),
                    tag: true,
                    required: true,
                };
                leaf(
                    offset,
                    self.blocks(core::slice::from_ref(&entry), &StateMatch::default()),
                )
            }
            SupportPredicate::MatchingFluids { offset, fluids } => {
                leaf(offset, self.fluids(fluids))
            }
            SupportPredicate::HasSturdyFace {
                offset,
                direction,
                face_test,
                shape,
            } => {
                let definitions = self.definitions;
                let set = self.states(|state| {
                    let shape = match shape {
                        FaceShape::Support => definitions.support_shape(state),
                        FaceShape::Collision => definitions.state(state).collision_shape,
                    };
                    face_test::passes(definitions.shape(shape), *direction, *face_test)
                });
                leaf(offset, set)
            }
            SupportPredicate::Solid { offset } => {
                let set = self.flagged(BlockStateFlags::LEGACY_SOLID);
                leaf(offset, set)
            }
            SupportPredicate::Replaceable { offset } => {
                let set = self.flagged(BlockStateFlags::REPLACEABLE);
                leaf(offset, set)
            }
            SupportPredicate::Not { predicate } => Node::Not(Box::new(self.predicate(predicate))),
            SupportPredicate::AnyOf { predicates } => Node::AnyOf(
                predicates
                    .iter()
                    .map(|predicate| self.predicate(predicate))
                    .collect(),
            ),
            SupportPredicate::AllOf { predicates } => Node::AllOf(
                predicates
                    .iter()
                    .map(|predicate| self.predicate(predicate))
                    .collect(),
            ),
            SupportPredicate::True => Node::True,
        }
    }

    fn flagged(&mut self, flag: BlockStateFlags) -> usize {
        let definitions = self.definitions;
        self.states(|state| definitions.state(state).flags.contains(flag))
    }

    fn blocks(&mut self, entries: &[TagEntry], state: &StateMatch) -> usize {
        let mut members: Vec<Id<Block>> = Vec::new();
        for entry in entries {
            if entry.tag {
                let tag = self
                    .tags
                    .and_then(|tags| tags.id(TagKey::new(entry.id.as_location())));
                match (self.tags, tag) {
                    (Some(tags), Some(tag)) => members.extend(tags.members(tag)),
                    _ => self.miss(Missing::BlockTag(entry.id.clone())),
                }
            } else {
                match self.definitions.id_of(entry.id.as_str()) {
                    Some(block) => members.push(block),
                    None => self.miss(Missing::Block(entry.id.clone())),
                }
            }
        }
        let definitions = self.definitions;
        self.states(|candidate| {
            let Some(block) = definitions.block_index(candidate) else {
                return false;
            };
            members.contains(&block)
                && definitions
                    .entry(block)
                    .is_some_and(|entry| matches_state(entry, candidate, state))
        })
    }

    fn fluids(&mut self, entries: &[TagEntry]) -> usize {
        let mut members: Vec<u16> = Vec::new();
        for entry in entries {
            let fluid = self
                .registries
                .entries::<Fluid>()
                .and_then(|fluids| fluids.by_name(entry.id.as_location()));
            match fluid {
                Some(fluid) if !entry.tag => members.push(fluid.number()),
                _ => self.miss(Missing::Fluid(entry.id.clone())),
            }
        }
        let definitions = self.definitions;
        self.states(|state| {
            definitions
                .state(state)
                .fluid
                .is_some_and(|fluid| members.contains(&fluid.fluid.number()))
        })
    }

    fn miss(&mut self, missing: Missing) {
        if let Some(block) = &self.block {
            self.issues.push(SupportIssue {
                block: block.clone(),
                missing,
            });
        }
    }

    /// The index of the set of the states that `matches` accepts, shared with an equal set.
    fn states(&mut self, matches: impl Fn(BlockStateId) -> bool) -> usize {
        let mut set = FixedBitSet::with_capacity(self.definitions.state_count());
        for state in self.definitions.states().filter(|state| matches(*state)) {
            set.insert(state.index());
        }
        if let Some(index) = self.sets.iter().position(|known| *known == set) {
            return index;
        }
        self.sets.push(set);
        self.sets.len() - 1
    }
}

fn matches_state(entry: &BlockEntry, state: BlockStateId, matcher: &StateMatch) -> bool {
    matcher.0.iter().all(|(property, wanted)| {
        let Some(value) = entry.value_of(state, property) else {
            return false;
        };
        match wanted {
            ValueMatch::Exact(text) => value.renders_to(text),
            ValueMatch::Range { min, max } => {
                let at_least = |bound: &Option<String>, sign: i32| {
                    bound
                        .as_deref()
                        .is_none_or(|bound| compare(value, bound) * sign >= 0)
                };
                at_least(min, 1) && at_least(max, -1)
            }
        }
    })
}

/// The order of a value against the text of a bound: as numbers when both are numbers, as text
/// otherwise.
fn compare(value: &PropertyValue, bound: &str) -> i32 {
    let text = value.to_text();
    let order = match (text.parse::<i64>(), bound.parse::<i64>()) {
        (Ok(value), Ok(bound)) => value.cmp(&bound),
        _ => text.as_str().cmp(bound),
    };
    order as i32
}
