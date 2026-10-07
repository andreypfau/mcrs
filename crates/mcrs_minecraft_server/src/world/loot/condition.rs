use mcrs_minecraft_item::component::ValueMatcher;
use mcrs_minecraft_item::component::predicate::{
    BlockPredicate, ComponentPredicate, ComponentPredicateEntry, EnchantmentPredicate,
    EnchantmentsPredicate, ItemPredicate,
};
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_loot::condition::EntityProperties;
use mcrs_minecraft_loot::{EntityTarget, LootCondition};
use mcrs_minecraft_registry::{Holder, HolderList, Id};

use crate::world::loot::LootRegistries;
use crate::world::loot::context::BlockBreakContext;

// chisle: the block-break context has no random source, position, tool item or
// entity beyond the player, so a condition that needs one does not hold. That is
// the outcome vanilla gives most often for a player breaking by hand: no shears,
// no lucky roll. A richer loot context (#70) lifts this.
const UNDECIDABLE: bool = false;

pub(crate) type Term<'a> = (Option<Id<LootCondition>>, &'a LootCondition);

pub fn holds(condition: Option<&Holder<LootCondition>>, ctx: &BlockBreakContext) -> bool {
    condition.is_none_or(|condition| check(ctx.loot.resolve(condition).1, ctx))
}

pub fn check(condition: &LootCondition, ctx: &BlockBreakContext) -> bool {
    match condition {
        LootCondition::Inverted(inverted) => !check(ctx.loot.resolve(&inverted.term).1, ctx),
        LootCondition::AnyOf(terms) => ctx
            .loot
            .terms(&terms.terms)
            .any(|(_, term)| check(term, ctx)),
        LootCondition::AllOf(terms) => ctx
            .loot
            .terms(&terms.terms)
            .all(|(_, term)| check(term, ctx)),
        LootCondition::MatchTool(tool) => tool
            .predicate
            .as_ref()
            .is_none_or(|predicate| tool_matches(predicate, ctx)),
        LootCondition::MatchBlock(predicate) => block_matches(predicate, ctx),
        LootCondition::SurvivesExplosion => true,
        LootCondition::EntityProperties(properties) => entity_matches(properties),
        _ => UNDECIDABLE,
    }
}

/// A block-break context is a player breaking the block: `this` is that player,
/// and no other entity is present.
fn entity_matches(properties: &EntityProperties) -> bool {
    properties.predicate.as_ref().is_none_or(|predicate| {
        properties.entity == EntityTarget::This && (predicate.is_empty() || UNDECIDABLE)
    })
}

fn tool_matches(predicate: &ItemPredicate, ctx: &BlockBreakContext) -> bool {
    (predicate.items.is_none() || UNDECIDABLE)
        && (predicate.count.is_any() || UNDECIDABLE)
        && (predicate.matchers.components.0.is_empty() || UNDECIDABLE)
        && predicate
            .matchers
            .predicates
            .0
            .iter()
            .all(|entry| match entry {
                ComponentPredicateEntry::Typed(ComponentPredicate::Enchantments(
                    EnchantmentsPredicate(required),
                )) => required
                    .iter()
                    .all(|required| enchantment_matches(required, ctx)),
                _ => UNDECIDABLE,
            })
}

fn enchantment_matches(required: &EnchantmentPredicate, ctx: &BlockBreakContext) -> bool {
    let held = ctx
        .tool_enchantments
        .map_or(&[][..], |enchantments| &enchantments.0[..]);
    let level_of = |id: Id<EnchantmentData>| {
        held.iter()
            .find(|(key, _)| ctx.loot.enchantments.get(key) == Some(id))
            .map_or(0, |(_, level)| *level)
    };
    match &required.enchantments {
        Some(set) => set
            .ids(&ctx.loot.enchantment_tags)
            .any(|id| required.levels.matches(level_of(id))),
        None if !required.levels.is_any() => held
            .iter()
            .any(|(_, level)| required.levels.matches(*level)),
        None => !held.is_empty(),
    }
}

fn block_matches(predicate: &BlockPredicate, ctx: &BlockBreakContext) -> bool {
    let block = ctx.blocks.owner(ctx.state);
    predicate
        .blocks
        .as_ref()
        .is_none_or(|set| set.contains(ctx.blocks.block_index(ctx.state), ctx.tags))
        && predicate.state.as_ref().is_none_or(|state| {
            state.0.iter().all(|(property, matcher)| match matcher {
                ValueMatcher::Exact(expected) => block
                    .value_of(ctx.state, property)
                    .is_some_and(|value| value.renders_to(expected)),
                ValueMatcher::Ranged { .. } => UNDECIDABLE,
            })
        })
        && (predicate.nbt.is_none() || UNDECIDABLE)
        && (predicate.matchers.is_empty() || UNDECIDABLE)
}

impl LootRegistries {
    pub(crate) fn resolve<'a>(&'a self, holder: &'a Holder<LootCondition>) -> Term<'a> {
        match holder {
            Holder::Direct(condition) => (None, condition),
            Holder::Reference(id) => (Some(*id), &self.predicates[*id]),
        }
    }

    pub(crate) fn terms<'a>(
        &'a self,
        list: &'a HolderList<LootCondition>,
    ) -> impl Iterator<Item = Term<'a>> + 'a {
        let (holders, tag) = match list {
            HolderList::Named(tag) => (&[][..], Some(*tag)),
            HolderList::One(holder) => (std::slice::from_ref(&**holder), None),
            HolderList::List(holders) => (&holders[..], None),
        };
        holders.iter().map(|holder| self.resolve(holder)).chain(
            tag.into_iter()
                .flat_map(|tag| self.predicate_tags.members(tag))
                .map(|id| (Some(id), &self.predicates[id])),
        )
    }

    fn subterms<'a>(&'a self, condition: &'a LootCondition) -> impl Iterator<Item = Term<'a>> + 'a {
        let (single, list) = match condition {
            LootCondition::Inverted(inverted) => (Some(&*inverted.term), None),
            LootCondition::AnyOf(terms) | LootCondition::AllOf(terms) => (None, Some(&terms.terms)),
            _ => (None, None),
        };
        single
            .into_iter()
            .map(|holder| self.resolve(holder))
            .chain(list.into_iter().flat_map(|list| self.terms(list)))
    }

    /// The first registered predicate that reaches itself through the ones it
    /// names, which would make checking it recurse without end.
    pub(crate) fn self_referring_predicate(&self) -> Option<Id<LootCondition>> {
        let mut stack = Vec::new();
        self.predicate_names.ids().find_map(|id| {
            stack.push(id);
            let found = self.reaches_stacked(&self.predicates[id], &mut stack);
            stack.pop();
            found
        })
    }

    fn reaches_stacked(
        &self,
        condition: &LootCondition,
        stack: &mut Vec<Id<LootCondition>>,
    ) -> Option<Id<LootCondition>> {
        self.subterms(condition).find_map(|(id, term)| match id {
            Some(id) if stack.contains(&id) => Some(id),
            Some(id) => {
                stack.push(id);
                let found = self.reaches_stacked(term, stack);
                stack.pop();
                found
            }
            None => self.reaches_stacked(term, stack),
        })
    }
}
