use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_key::item_keys::Item;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::draws::Draws;
use crate::loot_context::{LootContext, Tool};
use crate::number::Number;

/// A loot condition of a block table.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Condition {
    /// A condition of a type that no rule states.
    Never,
    Inverted(Box<Condition>),
    AnyOf(Box<[Condition]>),
    AllOf(Box<[Condition]>),
    SurvivesExplosion,
    RandomChance(Number),
    TableBonus {
        enchantment: Option<Id<Enchantment>>,
        chances: Box<[f32]>,
    },
    StateIn(StateSet),
    BlockAt {
        offset: BlockPos,
        states: Option<StateSet>,
    },
    MatchTool(ItemTest),
    ThisEntity,
}

impl Condition {
    pub(crate) fn test<W: Fn(BlockPos) -> Option<BlockStateId>>(
        &self,
        cx: &LootContext<'_, W>,
        draws: &mut Draws,
    ) -> bool {
        match self {
            Condition::Never => false,
            Condition::Inverted(term) => !term.test(cx, draws),
            Condition::AnyOf(terms) => terms.iter().any(|term| term.test(cx, draws)),
            Condition::AllOf(terms) => terms.iter().all(|term| term.test(cx, draws)),
            Condition::SurvivesExplosion => cx
                .explosion_radius
                .is_none_or(|radius| draws.unit() <= 1.0 / radius),
            Condition::RandomChance(chance) => {
                let chance = chance.float(draws);
                draws.unit() < chance
            }
            Condition::TableBonus {
                enchantment,
                chances,
            } => {
                let level = usize::try_from(cx.tool.level(*enchantment)).unwrap_or(usize::MAX);
                let last = chances.len().saturating_sub(1);
                chances
                    .get(level.min(last))
                    .is_some_and(|&chance| draws.unit() < chance)
            }
            Condition::StateIn(states) => states.contains(cx.state),
            Condition::BlockAt { offset, states } => states.as_ref().is_none_or(|states| {
                let at = BlockPos::new(
                    cx.origin.x.wrapping_add(offset.x),
                    cx.origin.y.wrapping_add(offset.y),
                    cx.origin.z.wrapping_add(offset.z),
                );
                (cx.block_at)(at).is_some_and(|state| states.contains(state))
            }),
            Condition::MatchTool(test) => test.matches(&cx.tool),
            Condition::ThisEntity => cx.this_entity,
        }
    }
}

/// The states a block predicate accepts, sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateSet(pub(crate) Box<[BlockStateId]>);

impl StateSet {
    fn contains(&self, state: BlockStateId) -> bool {
        self.0.binary_search(&state).is_ok()
    }
}

/// The parts of an item predicate that block tables use (LOOT-CND-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ItemTest {
    pub(crate) items: Option<Box<[Id<Item>]>>,
    pub(crate) enchantments: Box<[EnchantmentTest]>,
}

/// One element of the sub-predicate `minecraft:enchantments`: any of `any_of`, or any
/// enchantment when it is `None`, at a level in `min..=max`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnchantmentTest {
    pub(crate) any_of: Option<Box<[Id<Enchantment>]>>,
    pub(crate) min: u32,
    pub(crate) max: u32,
}

impl ItemTest {
    fn matches(&self, tool: &Tool<'_>) -> bool {
        let item_matches = match (&self.items, tool.item) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(items), Some(item)) => items.contains(&item),
        };
        item_matches
            && self.enchantments.iter().all(|test| {
                tool.enchantments.iter().any(|&(id, level)| {
                    level >= test.min.max(1)
                        && level <= test.max
                        && test.any_of.as_ref().is_none_or(|ids| ids.contains(&id))
                })
            })
    }
}
