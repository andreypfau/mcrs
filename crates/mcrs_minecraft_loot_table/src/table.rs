use mcrs_minecraft_item_stack::{ItemStack, Kind};
use mcrs_minecraft_loot_condition::{Condition, LootContext, NumberProvider, bounded};
use mcrs_minecraft_predicate::LocationView;
use mcrs_minecraft_random::Random;

use crate::loot_scratch::LootScratch;
use crate::modifier::Modifier;
use crate::work_stack::WorkStack;

/// A baked table. Its entries sit in one list; a composite entry names the span of its children
/// and each child names its parent, so a candidate is an index into the list.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    /// The value of the name of the table in the key of its keyed stream (LOOT-RND-001).
    pub(crate) name_key: u64,
    pub(crate) pools: Box<[Pool]>,
    pub(crate) entries: Box<[Entry]>,
    pub(crate) modifiers: Box<[Modifier]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pool {
    pub(crate) condition: Option<Condition>,
    pub(crate) rolls: NumberProvider,
    pub(crate) bonus_rolls: NumberProvider,
    pub(crate) entries: Span,
    pub(crate) modifiers: Box<[Modifier]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) condition: Option<Condition>,
    pub(crate) kind: EntryKind,
    pub(crate) modifiers: Box<[Modifier]>,
    pub(crate) weight: i32,
    pub(crate) parent: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EntryKind {
    Item(Kind),
    /// A leaf that makes nothing: `empty`, `dynamic`, or a type that no rule states.
    Nothing,
    Alternatives(Span),
    Sequence(Span),
    Group(Span),
}

/// The entries `start..end` of the list of a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: u32,
    pub(crate) end: u32,
}

impl Span {
    fn indices(self) -> core::ops::Range<u32> {
        self.start..self.end
    }
}

// chisle: no context holds the luck yet, so each pool reads `bonus_rolls` with the luck 0
// (LOOT-CTX-015); a luck in the context lifts it.
const LUCK: f32 = 0.0;

/// The parts of one evaluation that do not change while it runs.
struct Evaluation<'e, 'c, E, W> {
    table: &'e Table,
    context: &'e LootContext<'c, E>,
    world: &'e W,
    max_stack: &'e [u8],
}

impl Table {
    /// Adds the stacks of one evaluation to `stacks`, each divided at the maximum stack size of
    /// its item in `max_stack` (LOOT-EVL-001, LOOT-OUT-001).
    pub(crate) fn evaluate<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        max_stack: &[u8],
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        let evaluation = Evaluation {
            table: self,
            context,
            world,
            max_stack,
        };
        for pool in &self.pools {
            evaluation.pool(pool, random, scratch, stacks);
        }
    }

    fn entry(&self, index: u32) -> Option<&Entry> {
        self.entries.get(usize::try_from(index).ok()?)
    }
}

impl<E, W: LocationView> Evaluation<'_, '_, E, W> {
    /// LOOT-EVL-002.
    fn pool<R: Random>(
        &self,
        pool: &Pool,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        if !self.passes(pool.condition.as_ref(), random) {
            return;
        }
        let rolls = pool.rolls.read_int(self.context, self.world, random);
        let bonus_rolls = pool
            .bonus_rolls
            .read_float(self.context, self.world, random);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the cast saturates, and a count beyond the i32 range is no real roll count"
        )]
        let bonus = (bonus_rolls * LUCK).floor() as i32;
        for _ in 0..rolls.saturating_add(bonus) {
            scratch.candidates.clear();
            for index in pool.entries.indices() {
                self.expand(index, random, &mut scratch.candidates);
            }
            if let Some(chosen) = self.choose(&mut scratch.candidates, random) {
                self.make(chosen, pool, random, stacks);
            }
        }
    }

    /// Steps 2 to 7 of LOOT-EVL-003.
    fn choose<R: Random>(&self, candidates: &mut Vec<u32>, random: &mut R) -> Option<u32> {
        let weight = |index: u32| self.table.entry(index).map_or(0, |entry| entry.weight);
        candidates.retain(|&index| weight(index) > 0);
        match candidates.as_slice() {
            [] => None,
            [only] => Some(*only),
            many => {
                let total = many.iter().fold(0, |total: i64, &index| {
                    total.saturating_add(i64::from(weight(index)))
                });
                let mut pick = bounded(random, total)?;
                many.iter().copied().find(|&index| {
                    pick -= weight(index);
                    pick < 0
                })
            }
        }
    }

    /// Adds the leaf entries of the entry at `index` to `candidates` and says whether its
    /// expansion passed (LOOT-EVL-004).
    fn expand<R: Random>(&self, index: u32, random: &mut R, candidates: &mut Vec<u32>) -> bool {
        let Some(entry) = self.table.entry(index) else {
            return false;
        };
        if !self.passes(entry.condition.as_ref(), random) {
            return false;
        }
        match entry.kind {
            EntryKind::Item(_) | EntryKind::Nothing => {
                candidates.push(index);
                true
            }
            EntryKind::Alternatives(children) => children
                .indices()
                .any(|child| self.expand(child, random, candidates)),
            EntryKind::Sequence(children) => children
                .indices()
                .all(|child| self.expand(child, random, candidates)),
            EntryKind::Group(children) => {
                for child in children.indices() {
                    self.expand(child, random, candidates);
                }
                true
            }
        }
    }

    /// The stack of a selected leaf (LOOT-EVL-006), through its modifiers in the order of
    /// LOOT-EVL-011 and then the division (LOOT-EVL-012).
    fn make<R: Random>(
        &self,
        leaf: u32,
        pool: &Pool,
        random: &mut R,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        let Some(entry) = self.table.entry(leaf) else {
            return;
        };
        let EntryKind::Item(kind) = entry.kind else {
            return;
        };
        let mut count = self.apply(&entry.modifiers, 1, random);
        let mut parent = entry.parent;
        while let Some(composite) = parent.and_then(|index| self.table.entry(index)) {
            count = self.apply(&composite.modifiers, count, random);
            parent = composite.parent;
        }
        count = self.apply(&pool.modifiers, count, random);
        count = self.apply(&self.table.modifiers, count, random);
        let max = self
            .max_stack
            .get(usize::from(kind.0))
            .copied()
            .unwrap_or(1);
        WorkStack { kind, count }.divide(max, stacks);
    }

    fn apply<R: Random>(&self, modifiers: &[Modifier], count: i32, random: &mut R) -> i32 {
        modifiers.iter().fold(count, |count, modifier| {
            modifier.apply(count, self.context, self.world, random)
        })
    }

    fn passes<R: Random>(&self, condition: Option<&Condition>, random: &mut R) -> bool {
        condition.is_none_or(|condition| condition.test(self.context, self.world, random))
    }
}
