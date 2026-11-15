use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_loot_condition::{Condition, LootContext, NumberProvider, bounded};
use mcrs_minecraft_predicate::LocationView;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::Id;

use crate::modifier::Modifier;

/// A baked block table: its pools, its modifiers and the key of its random stream.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Table {
    pub(crate) key: u64,
    pub(crate) pools: Box<[Pool]>,
    pub(crate) modifiers: Box<[Modifier]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Pool {
    pub(crate) condition: Option<Condition>,
    pub(crate) rolls: NumberProvider,
    pub(crate) entries: Box<[Entry]>,
    pub(crate) modifiers: Box<[Modifier]>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub(crate) condition: Option<Condition>,
    pub(crate) kind: EntryKind,
    pub(crate) modifiers: Box<[Modifier]>,
    pub(crate) weight: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EntryKind {
    Item(Id<Item>),
    /// A leaf that makes nothing: `empty`, `dynamic`, or a type that no rule states.
    Nothing,
    Alternatives(Box<[Entry]>),
    Sequence(Box<[Entry]>),
    Group(Box<[Entry]>),
}

type Counts = Vec<(Id<Item>, i32)>;

impl Table {
    /// Adds the drop list of one evaluation to `out` (LOOT-EVL-001).
    pub(crate) fn evaluate<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
        out: &mut Counts,
    ) {
        for pool in &self.pools {
            let start = out.len();
            pool.evaluate(context, world, random, out);
            apply_all(
                &self.modifiers,
                out.get_mut(start..),
                context,
                world,
                random,
            );
        }
    }
}

impl Pool {
    fn evaluate<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
        out: &mut Counts,
    ) {
        if !passes(self.condition.as_ref(), context, world, random) {
            return;
        }
        let mut candidates = Vec::new();
        for _ in 0..self.rolls.read_int(context, world, random) {
            let start = out.len();
            candidates.clear();
            for entry in &self.entries {
                entry.expand(context, world, random, &mut candidates);
            }
            candidates.retain(|entry: &&Entry| entry.weight > 0);
            let chosen = match candidates.as_slice() {
                [] => None,
                [only] => Some(*only),
                many => {
                    let total = many.iter().fold(0, |total: i64, entry| {
                        total.saturating_add(i64::from(entry.weight))
                    });
                    bounded(random, total).and_then(|mut pick| {
                        many.iter().copied().find(|entry| {
                            pick -= entry.weight;
                            pick < 0
                        })
                    })
                }
            };
            if let Some(entry) = chosen {
                entry.make(context, world, random, out);
            }
            apply_all(
                &self.modifiers,
                out.get_mut(start..),
                context,
                world,
                random,
            );
        }
    }
}

impl Entry {
    /// Selects the leaf entries of this entry and says whether its expansion passed
    /// (LOOT-EVL-004).
    fn expand<'e, E, W: LocationView, R: Random>(
        &'e self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
        selected: &mut Vec<&'e Entry>,
    ) -> bool {
        if !passes(self.condition.as_ref(), context, world, random) {
            return false;
        }
        match &self.kind {
            EntryKind::Item(_) | EntryKind::Nothing => {
                selected.push(self);
                true
            }
            EntryKind::Alternatives(children) => children
                .iter()
                .any(|child| child.expand(context, world, random, selected)),
            EntryKind::Sequence(children) => children
                .iter()
                .all(|child| child.expand(context, world, random, selected)),
            EntryKind::Group(children) => {
                for child in children {
                    child.expand(context, world, random, selected);
                }
                true
            }
        }
    }

    /// The stacks of a leaf entry (LOOT-EVL-006).
    fn make<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
        out: &mut Counts,
    ) {
        if let EntryKind::Item(item) = self.kind {
            let count = self.modifiers.iter().fold(1, |count, modifier| {
                modifier.apply(count, context, world, random)
            });
            out.push((item, count));
        }
    }
}

fn passes<E, W: LocationView, R: Random>(
    condition: Option<&Condition>,
    context: &LootContext<'_, E>,
    world: &W,
    random: &mut R,
) -> bool {
    condition.is_none_or(|condition| condition.test(context, world, random))
}

fn apply_all<E, W: LocationView, R: Random>(
    modifiers: &[Modifier],
    stacks: Option<&mut [(Id<Item>, i32)]>,
    context: &LootContext<'_, E>,
    world: &W,
    random: &mut R,
) {
    for (_, count) in stacks.into_iter().flatten() {
        *count = modifiers.iter().fold(*count, |count, modifier| {
            modifier.apply(count, context, world, random)
        });
    }
}
