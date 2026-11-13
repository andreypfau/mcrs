use mcrs_minecraft_item::enchantment::Enchantment;
use mcrs_minecraft_resource::Id;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::condition::Condition;
use crate::draws::Draws;
use crate::loot_context::LootContext;
use crate::number::Number;

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
        count: Number,
        add: bool,
    },
    ExplosionDecay,
    Limit {
        min: Option<Number>,
        max: Option<Number>,
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
    pub(crate) fn apply<W: Fn(BlockPos) -> Option<BlockStateId>>(
        &self,
        count: i32,
        cx: &LootContext<'_, W>,
        draws: &mut Draws,
    ) -> i32 {
        if self
            .condition
            .as_ref()
            .is_some_and(|condition| !condition.test(cx, draws))
        {
            return count;
        }
        let level = |enchantment| i32::try_from(cx.tool.level(enchantment)).unwrap_or(i32::MAX);
        match &self.op {
            ModifierOp::Keep => count,
            ModifierOp::SetCount { count: value, add } => {
                let value = value.int(draws);
                if *add {
                    count.saturating_add(value)
                } else {
                    value
                }
            }
            ModifierOp::ExplosionDecay => match cx.explosion_radius {
                None => count,
                Some(radius) => (0..count)
                    .map(|_| i32::from(draws.unit() <= 1.0 / radius))
                    .sum(),
            },
            ModifierOp::Limit { min, max } => {
                let floor = min.as_ref().map(|min| min.int(draws));
                let ceiling = max.as_ref().map(|max| max.int(draws));
                let count = floor.map_or(count, |floor| count.max(floor));
                ceiling.map_or(count, |ceiling| count.min(ceiling))
            }
            ModifierOp::OreDrops(enchantment) => {
                let level = level(*enchantment);
                if level > 0 {
                    let factor = (draws.between(0, level.saturating_add(1)) - 1).max(0) + 1;
                    count.saturating_mul(factor)
                } else {
                    count
                }
            }
            ModifierOp::UniformBonus {
                enchantment,
                multiplier,
            } => count
                .saturating_add(draws.between(0, multiplier.saturating_mul(level(*enchantment)))),
            ModifierOp::BinomialBonus {
                enchantment,
                extra,
                probability,
            } => {
                let trials = level(*enchantment).saturating_add(*extra);
                count.saturating_add(draws.successes(trials, *probability))
            }
            ModifierOp::Sequence(modifiers) => modifiers
                .iter()
                .fold(count, |count, modifier| modifier.apply(count, cx, draws)),
        }
    }
}
