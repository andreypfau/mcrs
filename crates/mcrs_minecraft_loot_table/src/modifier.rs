use core::num::NonZeroU8;

use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_item_stack::{
    Damage, EnchantmentId, EnchantmentLevel, Enchantments, Kind, MaxDamage, Unbreakable,
};
use mcrs_minecraft_loot_condition::{
    Condition, ContextParam, ContextParams, NumberProvider, Tool, between, binomial, bounded,
};
use mcrs_minecraft_predicate::LocationView;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::Id;

use crate::table::Evaluation;
use crate::work_stack::WorkStack;

/// A baked item modifier.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Modifier {
    pub(crate) condition: Option<Condition>,
    pub(crate) op: ModifierOp,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModifierOp {
    /// A modifier whose name the bake could not resolve; it changes nothing.
    Unresolved,
    // chisle: `furnace_smelt`, `set_attributes`, `set_name`, `exploration_map`,
    // `set_stew_effect`, `copy_name`, `set_contents`, `modify_contents`, `filtered`,
    // `set_loot_table`, `set_lore`, `fill_player_head`, `copy_custom_data`, `copy_state`,
    // `set_banner_pattern`, `set_potion`, `set_random_dyes`, `set_random_potion`,
    // `set_instrument`, `copy_components`, the firework and book types, `toggle_tooltips`,
    // `set_ominous_bottle_amplifier`, `set_custom_model_data`, `set_components`,
    // `set_custom_data` and `enchant_with_levels` keep the stack. Of them, `enchant_with_levels`,
    // `set_attributes`, `set_stew_effect`, `set_contents`, `modify_contents`, `filtered`,
    // `set_random_dyes`, `set_random_potion`, `set_instrument`, `set_ominous_bottle_amplifier`
    // and `set_custom_model_data` skip the draws of section 9 of `loot-functions.md`, so a roll
    // after one of them is not exact; lifted by the evaluation of each type.
    Pending,
    SetCount {
        count: NumberProvider,
        add: bool,
    },
    ExplosionDecay,
    Limit {
        min: Option<NumberProvider>,
        max: Option<NumberProvider>,
    },
    OreDrops(Option<Id<Enchantment>>),
    UniformBonus {
        enchantment: Option<Id<Enchantment>>,
        multiplier: i32,
    },
    BinomialBonus {
        enchantment: Option<Id<Enchantment>>,
        extra: i32,
        probability: f32,
    },
    // chisle: the context holds no `attacking_entity`, so the modifier always stops at step 1
    // of LOOT-MOD-013; lifted by a context parameter that gives the equipment of the attacker.
    EnchantedCountIncrease,
    Discard,
    SetItem(Kind),
    SetDamage {
        damage: NumberProvider,
        add: bool,
    },
    EnchantRandomly {
        /// The source list (LOOT-MOD-018); `None` is each enchantment in registry order.
        options: Option<Box<[EnchantmentId]>>,
        only_compatible: bool,
    },
    SetEnchantments {
        levels: Box<[SetLevel]>,
        add: bool,
    },
    Sequence(Box<[Modifier]>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SetLevel {
    pub(crate) enchantment: EnchantmentId,
    pub(crate) level: NumberProvider,
}

impl ModifierOp {
    /// The parameters that the operation reads, without its condition and without the modifiers
    /// of a sequence, for the check of LOOT-CTX-013.
    pub(crate) fn reads(&self) -> ContextParams {
        fn providers<'a>(providers: impl Iterator<Item = &'a NumberProvider>) -> ContextParams {
            providers.fold(ContextParams::EMPTY, |reads, provider| {
                reads.union(provider.reads())
            })
        }
        match self {
            ModifierOp::Unresolved
            | ModifierOp::Pending
            | ModifierOp::Sequence(_)
            | ModifierOp::Discard
            | ModifierOp::SetItem(_) => ContextParams::EMPTY,
            ModifierOp::SetCount { count, .. } => count.reads(),
            ModifierOp::SetDamage { damage, .. } => damage.reads(),
            ModifierOp::ExplosionDecay => ContextParams::of(&[ContextParam::ExplosionRadius]),
            ModifierOp::Limit { min, max } => providers([min, max].into_iter().flatten()),
            ModifierOp::OreDrops(_)
            | ModifierOp::UniformBonus { .. }
            | ModifierOp::BinomialBonus { .. } => ContextParams::of(&[ContextParam::Tool]),
            ModifierOp::EnchantedCountIncrease => {
                ContextParams::of(&[ContextParam::AttackingEntity])
            }
            ModifierOp::EnchantRandomly { .. } => ContextParams::EMPTY,
            ModifierOp::SetEnchantments { levels, .. } => {
                providers(levels.iter().map(|level| &level.level))
            }
        }
    }
}

impl Modifier {
    /// Applies the modifier to `stack` with the draws of its rule (LOOT-MOD-009, LOOT-MOD-012).
    pub(crate) fn apply<E, W: LocationView, R: Random>(
        &self,
        stack: &mut WorkStack,
        evaluation: &Evaluation<'_, '_, E, W>,
        random: &mut R,
    ) {
        let context = evaluation.context;
        let world = evaluation.world;
        if self
            .condition
            .as_ref()
            .is_some_and(|condition| !condition.test(context, world, random))
        {
            return;
        }
        let count = stack.count();
        match &self.op {
            ModifierOp::Unresolved | ModifierOp::Pending | ModifierOp::EnchantedCountIncrease => {}
            ModifierOp::SetCount { count: value, add } => {
                let value = value.read_int(context, world, random);
                stack.count = if *add {
                    count.saturating_add(value)
                } else {
                    value
                };
            }
            ModifierOp::ExplosionDecay => {
                if let Some(radius) = context.explosion_radius {
                    let chance = 1.0 / radius;
                    stack.count = (0..count)
                        .map(|_| i32::from(random.next_f32() <= chance))
                        .sum();
                }
            }
            ModifierOp::Limit { min, max } => {
                let floor = min.as_ref().map(|min| min.read_int(context, world, random));
                let ceiling = max.as_ref().map(|max| max.read_int(context, world, random));
                stack.count = match (floor, ceiling) {
                    (None, None) => count,
                    (None, Some(ceiling)) => count.min(ceiling),
                    (Some(floor), None) => count.max(floor),
                    (Some(floor), Some(ceiling)) => count.max(floor).min(floor.max(ceiling)),
                };
            }
            ModifierOp::OreDrops(_)
            | ModifierOp::UniformBonus { .. }
            | ModifierOp::BinomialBonus { .. } => {
                if let Some(tool) = context.tool {
                    stack.count = apply_bonus(&self.op, tool, count, random);
                }
            }
            ModifierOp::Discard => stack.discard(),
            ModifierOp::SetItem(kind) => {
                if stack.read().is_some() {
                    stack.set_kind(*kind, &evaluation.rules.items);
                }
            }
            ModifierOp::SetDamage { damage, add } => {
                set_damage(stack, damage, *add, evaluation, random)
            }
            ModifierOp::EnchantRandomly {
                options,
                only_compatible,
            } => enchant_randomly(
                stack,
                options.as_deref(),
                *only_compatible,
                evaluation,
                random,
            ),
            ModifierOp::SetEnchantments { levels, add } => {
                set_enchantments(stack, levels, *add, evaluation, random);
            }
            ModifierOp::Sequence(modifiers) => {
                for modifier in modifiers {
                    modifier.apply(stack, evaluation, random);
                }
            }
        }
    }
}

/// LOOT-MOD-004: the count after the bonus of `op` with the `tool` of the context.
fn apply_bonus<R: Random>(op: &ModifierOp, tool: Tool<'_>, count: i32, random: &mut R) -> i32 {
    let level = |enchantment: Option<Id<Enchantment>>| {
        let level = enchantment.map_or(0, |enchantment| tool.level(enchantment));
        i32::try_from(level).unwrap_or(i32::MAX)
    };
    match op {
        ModifierOp::OreDrops(enchantment) => {
            let level = level(*enchantment);
            if level <= 0 {
                return count;
            }
            bounded(random, i64::from(level) + 2).map_or(count, |bonus| {
                count.saturating_mul(bonus.saturating_sub(1).max(0) + 1)
            })
        }
        ModifierOp::UniformBonus {
            enchantment,
            multiplier,
        } => {
            let bound = i64::from(*multiplier) * i64::from(level(*enchantment)) + 1;
            bounded(random, bound).map_or(count, |bonus| count.saturating_add(bonus))
        }
        ModifierOp::BinomialBonus {
            enchantment,
            extra,
            probability,
        } => {
            let trials = level(*enchantment).saturating_add(*extra);
            count.saturating_add(binomial(random, trials, *probability))
        }
        _ => count,
    }
}

/// LOOT-MOD-016, in f32.
fn set_damage<E, W: LocationView, R: Random>(
    stack: &mut WorkStack,
    damage: &NumberProvider,
    add: bool,
    evaluation: &Evaluation<'_, '_, E, W>,
    random: &mut R,
) {
    let items = &evaluation.rules.items;
    let Some(read) = stack.read() else {
        return;
    };
    if items.effective::<Unbreakable>(read).is_some() {
        return;
    }
    let (Some(&MaxDamage(max @ 0..)), Some(&Damage(current))) = (
        items.effective::<MaxDamage>(read),
        items.effective::<Damage>(read),
    ) else {
        return;
    };
    let value = damage.read_float(evaluation.context, evaluation.world, random);
    #[expect(clippy::cast_precision_loss, reason = "the rule calculates in f32")]
    let (max_f32, current_f32) = (max as f32, current.clamp(0, max) as f32);
    let base = if add {
        1.0 - current_f32 / max_f32
    } else {
        0.0
    };
    let part = 1.0 - (value + base).clamp(0.0, 1.0);
    #[expect(clippy::cast_possible_truncation, reason = "the cast saturates")]
    let written = (part * max_f32).floor() as i32;
    stack.set(Damage(written.clamp(0, max)), items);
}

/// The component that LOOT-MOD-018 raises, or `None` when it is not a typed component.
fn raised_enchantments(
    stack: &WorkStack,
    evaluation: &Evaluation<'_, '_, impl Sized, impl LocationView>,
) -> Option<Enchantments> {
    let read = stack.read()?;
    // chisle: `minecraft:stored_enchantments` has no typed component in the value-form stack, so
    // an enchanted book gets no enchantment; lifted when the stack crate types that component.
    if Some(read.kind) == evaluation.rules.enchanted_book {
        return None;
    }
    evaluation
        .rules
        .items
        .effective::<Enchantments>(read)
        .cloned()
}

/// Gives `enchantments` with `enchantment` at `level`; 0 removes it.
fn with_level(enchantments: &Enchantments, enchantment: EnchantmentId, level: u8) -> Enchantments {
    let others = enchantments
        .entries()
        .iter()
        .copied()
        .filter(|entry| entry.enchantment != enchantment);
    let new = NonZeroU8::new(level).map(|level| EnchantmentLevel { enchantment, level });
    Enchantments::new(others.chain(new)).unwrap_or_else(|| enchantments.clone())
}

fn level_of(enchantments: &Enchantments, enchantment: EnchantmentId) -> u8 {
    enchantments
        .entries()
        .iter()
        .find(|entry| entry.enchantment == enchantment)
        .map_or(0, |entry| entry.level.get())
}

/// LOOT-MOD-019.
fn enchant_randomly<E, W: LocationView, R: Random>(
    stack: &mut WorkStack,
    options: Option<&[EnchantmentId]>,
    only_compatible: bool,
    evaluation: &Evaluation<'_, '_, E, W>,
    random: &mut R,
) {
    let rules = evaluation.rules;
    let kind = stack.kind();
    let is_book = kind.is_some() && kind == rules.book;
    let candidates = || {
        let listed = options.into_iter().flatten().copied();
        let every = rules
            .enchantments
            .iter()
            .map(|rule| rule.id)
            .filter(|_| options.is_none());
        listed.chain(every).filter(move |&id| {
            !only_compatible
                || is_book
                || kind.is_some_and(|kind| {
                    rules
                        .enchantment(id)
                        .is_some_and(|rule| rule.supports(kind))
                })
        })
    };
    let length = candidates().count();
    let Some(index) = i64::try_from(length)
        .ok()
        .and_then(|length| bounded(random, length))
    else {
        return;
    };
    let Some(enchantment) = usize::try_from(index)
        .ok()
        .and_then(|index| candidates().nth(index))
    else {
        return;
    };
    let max_level = rules
        .enchantment(enchantment)
        .map_or(1, |rule| i32::try_from(rule.max_level).unwrap_or(i32::MAX));
    let level = between(random, 1, max_level).unwrap_or(1);
    if is_book && let Some(enchanted_book) = rules.enchanted_book {
        stack.replace(enchanted_book, 1);
    }
    raise(stack, enchantment, level, evaluation);
    // chisle: the context holds no `additional_cost_component_allowed`, so step 7 never applies;
    // lifted by that parameter and a typed `minecraft:additional_trade_cost`.
}

/// The raise of LOOT-MOD-018.
fn raise<E, W: LocationView>(
    stack: &mut WorkStack,
    enchantment: EnchantmentId,
    level: i32,
    evaluation: &Evaluation<'_, '_, E, W>,
) {
    let Some(enchantments) = raised_enchantments(stack, evaluation) else {
        return;
    };
    if level <= 0 {
        return;
    }
    let level = u8::try_from(level.min(255)).unwrap_or(u8::MAX);
    let level = level.max(level_of(&enchantments, enchantment));
    stack.set(
        with_level(&enchantments, enchantment, level),
        &evaluation.rules.items,
    );
}

/// LOOT-MOD-022.
fn set_enchantments<E, W: LocationView, R: Random>(
    stack: &mut WorkStack,
    levels: &[SetLevel],
    add: bool,
    evaluation: &Evaluation<'_, '_, E, W>,
    random: &mut R,
) {
    let rules = evaluation.rules;
    if stack.kind().is_some()
        && stack.kind() == rules.book
        && let Some(enchanted_book) = rules.enchanted_book
    {
        stack.set_kind(enchanted_book, &rules.items);
    }
    let Some(mut enchantments) = raised_enchantments(stack, evaluation) else {
        return;
    };
    for entry in levels {
        let value = entry
            .level
            .read_int(evaluation.context, evaluation.world, random);
        let current = i32::from(level_of(&enchantments, entry.enchantment));
        let new = if add {
            current.saturating_add(value)
        } else {
            value
        };
        let new = u8::try_from(new.clamp(0, 255)).unwrap_or(0);
        enchantments = with_level(&enchantments, entry.enchantment, new);
    }
    stack.set(enchantments, &rules.items);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_rules::ItemRules;
    use mcrs_minecraft_item_stack::Builtin;
    use mcrs_minecraft_item_table::ItemTable;
    use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
    use mcrs_minecraft_loot_condition::LootContext;
    use mcrs_minecraft_predicate::NoWorld;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    /// LOOT-MOD-004: without a `tool`, `apply_bonus` keeps the count and takes no draw.
    #[test]
    fn loot_mod_004_apply_bonus_without_a_tool_keeps_the_count_and_the_generator() {
        let rules = ItemRules {
            items: ItemTable::leak([Vec::<Builtin>::new()]),
            enchantments: Box::new([]),
            book: None,
            enchanted_book: None,
        };
        let context = LootContext::<()>::try_new(ContextKeySet::Empty).expect("a source key set");
        let evaluation = Evaluation {
            tables: &[],
            context: &context,
            world: &NoWorld,
            rules: &rules,
        };
        let enchantment = Id::checked(0, 1);
        for op in [
            ModifierOp::OreDrops(enchantment),
            ModifierOp::UniformBonus {
                enchantment,
                multiplier: 1,
            },
            ModifierOp::BinomialBonus {
                enchantment,
                extra: 3,
                probability: 0.5,
            },
        ] {
            let modifier = Modifier {
                condition: None,
                op,
            };
            let mut random = XoroshiroRandom::new(7);
            let before = random.clone();
            let mut stack = WorkStack::one(Kind(0));
            stack.count = 2;
            modifier.apply(&mut stack, &evaluation, &mut random);
            assert_eq!((stack.count, &random), (2, &before), "{modifier:?}");
        }
    }
}
