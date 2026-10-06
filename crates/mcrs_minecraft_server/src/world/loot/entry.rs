use mcrs_minecraft_loot::entry::Composite;
use mcrs_minecraft_loot::{IntExpression, LootCondition, LootPoolEntry, LootTableBody};
use mcrs_minecraft_registry::Holder;

use crate::world::loot::condition::holds;
use crate::world::loot::context::{BlockBreakContext, LootDrop};

pub fn roll(table: &LootTableBody, ctx: &BlockBreakContext) -> Vec<LootDrop> {
    let mut drops = Vec::new();
    let mut candidates = Vec::new();
    for pool in &table.pools {
        if !holds(pool.condition.as_ref(), ctx) {
            continue;
        }
        for _ in 0..rolls(&pool.rolls) {
            candidates.clear();
            for entry in &pool.entries {
                expand(entry, ctx, &mut candidates);
            }
            // chisle: with no random source the first of several candidates is
            // taken where the game picks one by weight; seeded rolls (#72) lift this.
            if let Some(LootPoolEntry::Item(item)) = candidates.first() {
                drops.push(LootDrop {
                    item: item.name,
                    count: 1,
                });
            }
        }
    }
    drops
}

// chisle: only a constant roll count is honoured and any other expression rolls
// once; number providers evaluated over the loot context (#72) lift this.
fn rolls(rolls: &Holder<IntExpression>) -> i32 {
    match rolls {
        Holder::Direct(IntExpression::Constant(count)) => *count,
        _ => 1,
    }
}

/// Collects the singleton entries `entry` expands to, and reports whether its
/// own condition held, which is what alternatives and sequences compose.
fn expand<'a>(
    entry: &'a LootPoolEntry,
    ctx: &BlockBreakContext,
    candidates: &mut Vec<&'a LootPoolEntry>,
) -> bool {
    let composite = |composite: &Composite| holds(composite.condition.as_ref(), ctx);
    match entry {
        LootPoolEntry::Alternatives(alternatives) => {
            composite(alternatives)
                && alternatives
                    .children
                    .iter()
                    .any(|child| expand(child, ctx, candidates))
        }
        LootPoolEntry::Sequence(sequence) => {
            composite(sequence)
                && sequence
                    .children
                    .iter()
                    .all(|child| expand(child, ctx, candidates))
        }
        LootPoolEntry::Group(group) => {
            if !composite(group) {
                return false;
            }
            for child in &group.children {
                expand(child, ctx, candidates);
            }
            true
        }
        singleton => {
            let (weight, condition) = singleton_terms(singleton);
            if !holds(condition, ctx) {
                return false;
            }
            if weight > 0 {
                candidates.push(singleton);
            }
            true
        }
    }
}

fn singleton_terms(entry: &LootPoolEntry) -> (i32, Option<&Holder<LootCondition>>) {
    match entry {
        LootPoolEntry::Empty(e) => (e.weight, e.condition.as_ref()),
        LootPoolEntry::Item(e) => (e.weight, e.condition.as_ref()),
        LootPoolEntry::LootTable(e) => (e.weight, e.condition.as_ref()),
        LootPoolEntry::Dynamic(e) => (e.weight, e.condition.as_ref()),
        LootPoolEntry::Tag(e) => (e.weight, e.condition.as_ref()),
        LootPoolEntry::Slots(e) => (e.weight, e.condition.as_ref()),
        LootPoolEntry::Alternatives(e) | LootPoolEntry::Sequence(e) | LootPoolEntry::Group(e) => {
            (1, e.condition.as_ref())
        }
    }
}
