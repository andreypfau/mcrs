use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_loot_condition::{Condition, LootContext, NumberProvider, binomial, bounded};
use mcrs_minecraft_predicate::LocationView;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::Id;

/// An item modifier of a block table. It changes the count of one stack.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Modifier {
    pub(crate) condition: Option<Condition>,
    pub(crate) op: ModifierOp,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModifierOp {
    /// A modifier of a type that changes nothing a stack here holds.
    Keep,
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
    Sequence(Box<[Modifier]>),
}

impl Modifier {
    pub(crate) fn apply<E, W: LocationView, R: Random>(
        &self,
        count: i32,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
    ) -> i32 {
        if self
            .condition
            .as_ref()
            .is_some_and(|condition| !condition.test(context, world, random))
        {
            return count;
        }
        let level = |enchantment: Option<Id<Enchantment>>| {
            let level = context
                .tool
                .zip(enchantment)
                .map_or(0, |(tool, enchantment)| tool.level(enchantment));
            i32::try_from(level).unwrap_or(i32::MAX)
        };
        match &self.op {
            ModifierOp::Keep => count,
            ModifierOp::SetCount { count: value, add } => {
                let value = value.read_int(context, world, random);
                if *add {
                    count.saturating_add(value)
                } else {
                    value
                }
            }
            ModifierOp::ExplosionDecay => match context.explosion_radius {
                None => count,
                Some(radius) => (0..count)
                    .map(|_| i32::from(random.next_f32() <= 1.0 / radius))
                    .sum(),
            },
            ModifierOp::Limit { min, max } => {
                let floor = min.as_ref().map(|min| min.read_int(context, world, random));
                let ceiling = max.as_ref().map(|max| max.read_int(context, world, random));
                let count = floor.map_or(count, |floor| count.max(floor));
                ceiling.map_or(count, |ceiling| count.min(ceiling))
            }
            ModifierOp::OreDrops(_)
            | ModifierOp::UniformBonus { .. }
            | ModifierOp::BinomialBonus { .. }
                if context.tool.is_none() =>
            {
                count
            }
            ModifierOp::OreDrops(enchantment) => {
                let level = level(*enchantment);
                if level <= 0 {
                    return count;
                }
                match bounded(random, i64::from(level) + 2) {
                    Some(bonus) => count.saturating_mul(bonus.saturating_sub(1).max(0) + 1),
                    None => count,
                }
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
            ModifierOp::Sequence(modifiers) => modifiers.iter().fold(count, |count, modifier| {
                modifier.apply(count, context, world, random)
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_predicate::NoWorld;
    use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;

    /// LOOT-MOD-004: without a `tool`, `apply_bonus` keeps the count and takes no draw.
    #[test]
    fn loot_mod_004_apply_bonus_without_a_tool_keeps_the_count_and_the_generator() {
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
            let count = modifier.apply(2, &LootContext::<()>::new(), &NoWorld, &mut random);
            assert_eq!((count, &random), (2, &before), "{modifier:?}");
        }
    }
}
