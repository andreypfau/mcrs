use mcrs_minecraft_item_stack::{ItemStack, Kind};
use mcrs_minecraft_loot_condition::{
    Condition, DynamicName, FloatProvider, IntProvider, LootContext, LootInputs, LootWorld,
    NamedEntries, bounded,
};
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;

use crate::item_rules::ItemRules;
use crate::loot_scratch::LootScratch;
use crate::modifier::{Modifier, apply_all};
use crate::work_stack::WorkStack;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    pub(crate) name_key: u64,
    pub(crate) pools: Box<[Pool]>,
    pub(crate) entries: Box<[Entry]>,
    pub(crate) items: Box<[Kind]>,
    pub(crate) nested: Box<[Option<u32>]>,
    pub(crate) modifiers: Box<[Modifier]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pool {
    pub(crate) condition: Option<Condition>,
    pub(crate) rolls: IntProvider,
    pub(crate) bonus_rolls: FloatProvider,
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
    Tag(Span),
    ExpandedTag(Span),
    Tables(Span),
    ExpandedTables(Span),
    Dynamic(DynamicName),
    Alternatives(Span),
    Sequence(Span),
    Group(Span),
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Candidate {
    entry: u32,
    member: Option<u32>,
    weight: i32,
}

pub(crate) struct Evaluation<'e, 'c, E, W> {
    pub(crate) tables: &'e [Table],
    pub(crate) named: &'e NamedEntries,
    pub(crate) context: &'e LootContext<'c, E>,
    pub(crate) world: &'e W,
    pub(crate) rules: &'e ItemRules,
}

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

    pub(crate) fn nested(&self, index: u32) -> Option<u32> {
        self.nested
            .get(usize::try_from(index).ok()?)
            .copied()
            .flatten()
    }
}

impl<'e, 'c, E: Copy, W: LootWorld<E>> Evaluation<'e, 'c, E, W> {
    pub(crate) fn inputs(&self) -> LootInputs<'e, 'c, E, W> {
        LootInputs {
            named: self.named,
            context: self.context,
            world: self.world,
        }
    }

    pub(crate) fn root<R: Random>(
        &self,
        table: &Table,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        self.table(table, None, random, scratch, stacks);
    }

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

    fn pool<R: Random>(
        &self,
        table: &Table,
        pool: &Pool,
        outer: Option<&Place<'_>>,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        if !self.passes(pool.condition.as_ref(), random, &mut scratch.predicates) {
            return;
        }
        let rolls = pool
            .rolls
            .read(self.inputs(), random, &mut scratch.predicates);
        let bonus_rolls = pool
            .bonus_rolls
            .read(self.inputs(), random, &mut scratch.predicates);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the cast saturates, and a count beyond the i32 range is no real roll count"
        )]
        let bonus = (bonus_rolls * self.context.luck).floor() as i32;
        for _ in 0..rolls.saturating_add(bonus) {
            scratch.candidates.clear();
            for index in pool.entries.indices() {
                self.expand(table, index, random, scratch);
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

    fn expand<R: Random>(
        &self,
        table: &Table,
        index: u32,
        random: &mut R,
        scratch: &mut LootScratch,
    ) -> bool {
        let Some(entry) = table.entry(index) else {
            return false;
        };
        if !self.passes(entry.condition.as_ref(), random, &mut scratch.predicates) {
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
                scratch.candidates.push(candidate(None));
                true
            }
            EntryKind::ExpandedTag(members) | EntryKind::ExpandedTables(members) => {
                scratch
                    .candidates
                    .extend(members.indices().map(|member| candidate(Some(member))));
                true
            }
            EntryKind::Alternatives(children) => children
                .indices()
                .any(|child| self.expand(table, child, random, scratch)),
            EntryKind::Sequence(children) => children
                .indices()
                .all(|child| self.expand(table, child, random, scratch)),
            EntryKind::Group(children) => {
                for child in children.indices() {
                    self.expand(table, child, random, scratch);
                }
                true
            }
        }
    }

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
            EntryKind::Item(kind) => {
                self.emit(place, WorkStack::one(*kind), random, scratch, stacks)
            }
            EntryKind::Tag(members) => {
                for kind in members.indices().filter_map(|index| table.item(index)) {
                    self.emit(place, WorkStack::one(kind), random, scratch, stacks);
                }
            }
            EntryKind::ExpandedTag(_) => {
                if let Some(kind) = member.and_then(|index| table.item(index)) {
                    self.emit(place, WorkStack::one(kind), random, scratch, stacks);
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
                    self.emit(
                        place,
                        WorkStack::copy(stack.as_ref()),
                        random,
                        scratch,
                        stacks,
                    );
                }
            }
            EntryKind::Empty
            | EntryKind::Alternatives(_)
            | EntryKind::Sequence(_)
            | EntryKind::Group(_) => {}
        }
    }

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

    fn emit<R: Random>(
        &self,
        place: &Place<'_>,
        mut stack: WorkStack,
        random: &mut R,
        scratch: &mut LootScratch,
        stacks: &mut Vec<Option<ItemStack>>,
    ) {
        let mut at = Some(place);
        while let Some(place) = at {
            self.modify(place, &mut stack, random, &mut scratch.predicates);
            at = place.outer;
        }
        stack.divide(&self.rules.items, stacks);
    }

    fn modify<R: Random>(
        &self,
        place: &Place<'_>,
        stack: &mut WorkStack,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) {
        let Some(entry) = place.table.entry(place.leaf) else {
            return;
        };
        let table = place.table;
        apply_all(&entry.modifiers, table, stack, self, random, scratch);
        let mut parent = entry.parent;
        while let Some(composite) = parent.and_then(|index| table.entry(index)) {
            apply_all(&composite.modifiers, table, stack, self, random, scratch);
            parent = composite.parent;
        }
        apply_all(&place.pool.modifiers, table, stack, self, random, scratch);
        apply_all(&table.modifiers, table, stack, self, random, scratch);
    }

    fn passes<R: Random>(
        &self,
        condition: Option<&Condition>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> bool {
        condition.is_none_or(|condition| condition.test(self.inputs(), random, scratch))
    }
}

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
