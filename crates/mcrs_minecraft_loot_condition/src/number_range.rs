use core::cmp::Ordering;

use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;

use crate::float_provider::FloatProvider;
use crate::int_provider::IntProvider;
use crate::loot_inputs::LootInputs;
use crate::loot_world::LootWorld;

/// A baked int range or float range (LOOT-CND-021).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NumberRange<P> {
    Point(P),
    Bounds { min: Option<P>, max: Option<P> },
}

/// A provider that a range reads, with the read of LOOT-NUM-001.
pub(crate) trait RangeProvider {
    type Value: PartialOrd + Copy;

    fn read_value<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> Self::Value;
}

impl RangeProvider for IntProvider {
    type Value = i32;

    fn read_value<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> i32 {
        self.read(inputs, random, scratch)
    }
}

impl RangeProvider for FloatProvider {
    type Value = f32;

    fn read_value<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> f32 {
        self.read(inputs, random, scratch)
    }
}

impl<P> NumberRange<P> {
    pub(crate) fn providers(&self) -> impl Iterator<Item = &P> {
        let (first, second) = match self {
            NumberRange::Point(point) => (Some(point), None),
            NumberRange::Bounds { min, max } => (min.as_ref(), max.as_ref()),
        };
        first.into_iter().chain(second)
    }
}

impl<P: RangeProvider> NumberRange<P> {
    /// Whether the read of `input` is in the range; a range with neither bound does not read
    /// `input` (LOOT-CND-021).
    pub(crate) fn contains_read<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        input: &P,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> bool {
        if let NumberRange::Bounds {
            min: None,
            max: None,
        } = self
        {
            return true;
        }
        let value = input.read_value(inputs, random, scratch);
        self.contains(value, inputs, random, scratch)
    }

    /// Whether `value` is in the range; the range reads `min` before `max`, and does not read
    /// `max` when `value` is less than `min` (LOOT-CND-021).
    pub(crate) fn contains<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        value: P::Value,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let (min, max) = match self {
            NumberRange::Point(point) => return value == point.read_value(inputs, random, scratch),
            NumberRange::Bounds { min, max } => (min, max),
        };
        if let Some(min) = min
            && value.partial_cmp(&min.read_value(inputs, random, scratch)) == Some(Ordering::Less)
        {
            return false;
        }
        max.as_ref().is_none_or(|max| {
            value.partial_cmp(&max.read_value(inputs, random, scratch)) != Some(Ordering::Greater)
        })
    }
}
