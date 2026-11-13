use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::condition::Condition;
use crate::draws::Draws;
use crate::loot_context::LootContext;
use crate::modifier::Modifier;
use crate::number::Number;

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
    pub(crate) rolls: Number,
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
    pub(crate) fn evaluate<W: Fn(BlockPos) -> Option<BlockStateId>>(
        &self,
        cx: &LootContext<'_, W>,
        draws: &mut Draws,
        out: &mut Counts,
    ) {
        for pool in &self.pools {
            let start = out.len();
            pool.evaluate(cx, draws, out);
            apply_all(&self.modifiers, out.get_mut(start..), cx, draws);
        }
    }
}

impl Pool {
    fn evaluate<W: Fn(BlockPos) -> Option<BlockStateId>>(
        &self,
        cx: &LootContext<'_, W>,
        draws: &mut Draws,
        out: &mut Counts,
    ) {
        if !passes(self.condition.as_ref(), cx, draws) {
            return;
        }
        let mut candidates = Vec::new();
        for _ in 0..self.rolls.int(draws) {
            let start = out.len();
            candidates.clear();
            for entry in &self.entries {
                entry.expand(cx, draws, &mut candidates);
            }
            candidates.retain(|entry: &&Entry| entry.weight > 0);
            let chosen = match candidates.as_slice() {
                [] => None,
                [only] => Some(*only),
                many => {
                    let total = many
                        .iter()
                        .fold(0, |total: i32, entry| total.saturating_add(entry.weight));
                    let mut pick = draws.between(0, total - 1);
                    many.iter().copied().find(|entry| {
                        pick -= entry.weight;
                        pick < 0
                    })
                }
            };
            if let Some(entry) = chosen {
                entry.make(cx, draws, out);
            }
            apply_all(&self.modifiers, out.get_mut(start..), cx, draws);
        }
    }
}

impl Entry {
    /// Selects the leaf entries of this entry and says whether its expansion passed
    /// (LOOT-EVL-004).
    fn expand<'e, W: Fn(BlockPos) -> Option<BlockStateId>>(
        &'e self,
        cx: &LootContext<'_, W>,
        draws: &mut Draws,
        selected: &mut Vec<&'e Entry>,
    ) -> bool {
        if !passes(self.condition.as_ref(), cx, draws) {
            return false;
        }
        match &self.kind {
            EntryKind::Item(_) | EntryKind::Nothing => {
                selected.push(self);
                true
            }
            EntryKind::Alternatives(children) => children
                .iter()
                .any(|child| child.expand(cx, draws, selected)),
            EntryKind::Sequence(children) => children
                .iter()
                .all(|child| child.expand(cx, draws, selected)),
            EntryKind::Group(children) => {
                for child in children {
                    child.expand(cx, draws, selected);
                }
                true
            }
        }
    }

    /// The stacks of a leaf entry (LOOT-EVL-006).
    fn make<W: Fn(BlockPos) -> Option<BlockStateId>>(
        &self,
        cx: &LootContext<'_, W>,
        draws: &mut Draws,
        out: &mut Counts,
    ) {
        if let EntryKind::Item(item) = self.kind {
            let count = self
                .modifiers
                .iter()
                .fold(1, |count, modifier| modifier.apply(count, cx, draws));
            out.push((item, count));
        }
    }
}

fn passes<W: Fn(BlockPos) -> Option<BlockStateId>>(
    condition: Option<&Condition>,
    cx: &LootContext<'_, W>,
    draws: &mut Draws,
) -> bool {
    condition.is_none_or(|condition| condition.test(cx, draws))
}

fn apply_all<W: Fn(BlockPos) -> Option<BlockStateId>>(
    modifiers: &[Modifier],
    stacks: Option<&mut [(Id<Item>, i32)]>,
    cx: &LootContext<'_, W>,
    draws: &mut Draws,
) {
    for (_, count) in stacks.into_iter().flatten() {
        *count = modifiers
            .iter()
            .fold(*count, |count, modifier| modifier.apply(count, cx, draws));
    }
}
