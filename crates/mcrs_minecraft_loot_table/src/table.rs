use mcrs_minecraft_item_stack::{ItemStack, Kind};
use mcrs_minecraft_loot_condition::{Condition, DynamicName, LootContext, NumberProvider, bounded};
use mcrs_minecraft_predicate::LocationView;
use mcrs_minecraft_random::Random;

use crate::loot_scratch::LootScratch;
use crate::modifier::Modifier;
use crate::work_stack::WorkStack;

/// A baked table. Its entries sit in one list; a composite entry names the span of its children
/// and each child names its parent, so a candidate is an index into the list. The members of
/// its `tag` entries and the tables of its `loot_table` entries sit in two more lists.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    /// The value of the name of the table in the key of its keyed stream (LOOT-RND-001).
    pub(crate) name_key: u64,
    pub(crate) pools: Box<[Pool]>,
    pub(crate) entries: Box<[Entry]>,
    pub(crate) items: Box<[Kind]>,
    /// The index of each nested table in the list of the load; `None` for a reference that the
    /// bake could not resolve or that closes a cycle (LOOT-CTX-014).
    pub(crate) nested: Box<[Option<u32>]>,
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
    pub(crate) quality: i32,
    pub(crate) parent: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EntryKind {
    Item(Kind),
    Empty,
    /// A `tag` entry without `expand`: one candidate that makes each item of the span of
    /// `items`.
    Tag(Span),
    /// A `tag` entry with `expand`: one candidate for each item of the span.
    ExpandedTag(Span),
    /// A `loot_table` entry without `expand`, over a span of `nested`.
    Tables(Span),
    ExpandedTables(Span),
    Dynamic(DynamicName),
    Alternatives(Span),
    Sequence(Span),
    Group(Span),
}

/// The elements `start..end` of one list of a table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: u32,
    pub(crate) end: u32,
}

impl Span {
    fn indices(self) -> core::ops::Range<u32> {
        self.start..self.end
    }
}

/// A leaf result of an expansion (LOOT-EVL-003). `member` is the index of the item or the
/// nested table of an expanded entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Candidate {
    entry: u32,
    member: Option<u32>,
    weight: i32,
}

/// The parts of one evaluation that do not change while it runs.
pub(crate) struct Evaluation<'e, 'c, E, W> {
    pub(crate) tables: &'e [Table],
    pub(crate) context: &'e LootContext<'c, E>,
    pub(crate) world: &'e W,
    pub(crate) max_stack: &'e [u8],
}

/// The selected leaf whose stacks the evaluation makes, and the place of its table in the
/// `loot_table` entry that evaluates it.
#[derive(Clone, Copy)]
struct Place<'p> {
    table: &'p Table,
    pool: &'p Pool,
    leaf: u32,
    outer: Option<&'p Place<'p>>,
}

impl Table {
    fn entry(&self, index: u32) -> Option<&Entry> {
        self.entries.get(usize::try_from(index).ok()?)
    }

    fn item(&self, index: u32) -> Option<Kind> {
        self.items.get(usize::try_from(index).ok()?).copied()
    }

    fn nested(&self, index: u32) -> Option<u32> {
        self.nested
            .get(usize::try_from(index).ok()?)
            .copied()
            .flatten()
    }
}

impl<E, W: LocationView> Evaluation<'_, '_, E, W> {
    /// Adds the stacks of one evaluation of `table`, which is not nested, to `stacks`
    /// (LOOT-EVL-001).
    pub(crate) fn root<R: Random>(
        &self,
        table: &Table,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        self.table(table, None, random, scratch, stacks);
    }

    /// Without `outer` each stack is divided (LOOT-OUT-001); a nested table hands each stack to
    /// the modifiers around the entry that evaluates it.
    fn table<R: Random>(
        &self,
        table: &Table,
        outer: Option<&Place<'_>>,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        for pool in &table.pools {
            self.pool(table, pool, outer, random, scratch, stacks);
        }
    }

    /// LOOT-EVL-002.
    fn pool<R: Random>(
        &self,
        table: &Table,
        pool: &Pool,
        outer: Option<&Place<'_>>,
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
        let bonus = (bonus_rolls * self.context.luck).floor() as i32;
        for _ in 0..rolls.saturating_add(bonus) {
            scratch.candidates.clear();
            for index in pool.entries.indices() {
                self.expand(table, index, random, &mut scratch.candidates);
            }
            if let Some(chosen) = choose(&mut scratch.candidates, random) {
                let place = Place {
                    table,
                    pool,
                    leaf: chosen.entry,
                    outer,
                };
                self.make(&place, chosen.member, random, scratch, stacks);
            }
        }
    }

    /// Adds the leaf results of the entry at `index` to `candidates` and says whether its
    /// expansion passed (LOOT-EVL-004).
    fn expand<R: Random>(
        &self,
        table: &Table,
        index: u32,
        random: &mut R,
        candidates: &mut Vec<Candidate>,
    ) -> bool {
        let Some(entry) = table.entry(index) else {
            return false;
        };
        if !self.passes(entry.condition.as_ref(), random) {
            return false;
        }
        let candidate = |member| Candidate {
            entry: index,
            member,
            weight: self.weight(entry),
        };
        match entry.kind {
            EntryKind::Item(_)
            | EntryKind::Empty
            | EntryKind::Tag(_)
            | EntryKind::Tables(_)
            | EntryKind::Dynamic(_) => {
                candidates.push(candidate(None));
                true
            }
            EntryKind::ExpandedTag(members) | EntryKind::ExpandedTables(members) => {
                candidates.extend(members.indices().map(|member| candidate(Some(member))));
                true
            }
            EntryKind::Alternatives(children) => children
                .indices()
                .any(|child| self.expand(table, child, random, candidates)),
            EntryKind::Sequence(children) => children
                .indices()
                .all(|child| self.expand(table, child, random, candidates)),
            EntryKind::Group(children) => {
                for child in children.indices() {
                    self.expand(table, child, random, candidates);
                }
                true
            }
        }
    }

    /// Step 2 of LOOT-EVL-003, in 32-bit floats.
    fn weight(&self, entry: &Entry) -> i32 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "the rule adds the weight and the quality as 32-bit floats"
        )]
        let weight = entry.quality as f32 * self.context.luck + entry.weight as f32;
        #[expect(clippy::cast_possible_truncation, reason = "the cast saturates")]
        let weight = weight.floor() as i32;
        weight.max(0)
    }

    /// The stacks of a selected leaf (LOOT-EVL-006), one at a time (LOOT-EVL-012).
    fn make<R: Random>(
        &self,
        place: &Place<'_>,
        member: Option<u32>,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        let table = place.table;
        let Some(entry) = table.entry(place.leaf) else {
            return;
        };
        match &entry.kind {
            EntryKind::Item(kind) => self.emit(place, WorkStack::one(*kind), random, stacks),
            EntryKind::Tag(members) => {
                for kind in members.indices().filter_map(|index| table.item(index)) {
                    self.emit(place, WorkStack::one(kind), random, stacks);
                }
            }
            EntryKind::ExpandedTag(_) => {
                if let Some(kind) = member.and_then(|index| table.item(index)) {
                    self.emit(place, WorkStack::one(kind), random, stacks);
                }
            }
            EntryKind::Tables(members) => {
                for index in members.indices() {
                    self.nested(place, table.nested(index), random, scratch, stacks);
                }
            }
            EntryKind::ExpandedTables(_) => {
                let nested = member.and_then(|index| table.nested(index));
                self.nested(place, nested, random, scratch, stacks);
            }
            EntryKind::Dynamic(name) => {
                for stack in self.context.dynamic_name(*name) {
                    self.emit(place, WorkStack::copy(stack.as_ref()), random, stacks);
                }
            }
            EntryKind::Empty
            | EntryKind::Alternatives(_)
            | EntryKind::Sequence(_)
            | EntryKind::Group(_) => {}
        }
    }

    /// LOOT-EVL-010: a nested table rolls in the same context, with the same generator.
    fn nested<R: Random>(
        &self,
        place: &Place<'_>,
        nested: Option<u32>,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        let table = nested
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| self.tables.get(index));
        if let Some(table) = table {
            self.table(table, Some(place), random, scratch, stacks);
        }
    }

    /// Puts `stack` through its modifiers in the order of LOOT-EVL-011, out to the table that
    /// is not nested, and then through the division (LOOT-OUT-001).
    fn emit<R: Random>(
        &self,
        place: &Place<'_>,
        mut stack: WorkStack<'_>,
        random: &mut R,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        let mut at = Some(place);
        while let Some(place) = at {
            stack.count = self.modify(place, stack.count, random);
            at = place.outer;
        }
        stack.divide(stack.max_stack_size(self.max_stack), stacks);
    }

    fn modify<R: Random>(&self, place: &Place<'_>, count: i32, random: &mut R) -> i32 {
        let Some(entry) = place.table.entry(place.leaf) else {
            return count;
        };
        let mut count = self.apply(&entry.modifiers, count, random);
        let mut parent = entry.parent;
        while let Some(composite) = parent.and_then(|index| place.table.entry(index)) {
            count = self.apply(&composite.modifiers, count, random);
            parent = composite.parent;
        }
        count = self.apply(&place.pool.modifiers, count, random);
        self.apply(&place.table.modifiers, count, random)
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

/// Steps 3 to 7 of LOOT-EVL-003.
fn choose<R: Random>(candidates: &mut Vec<Candidate>, random: &mut R) -> Option<Candidate> {
    candidates.retain(|candidate| candidate.weight > 0);
    match candidates.as_slice() {
        [] => None,
        [only] => Some(*only),
        many => {
            let total = many.iter().fold(0, |total: i64, candidate| {
                total.saturating_add(i64::from(candidate.weight))
            });
            let mut pick = bounded(random, total)?;
            many.iter().copied().find(|candidate| {
                pick -= candidate.weight;
                pick < 0
            })
        }
    }
}
