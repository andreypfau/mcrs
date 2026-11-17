use core::num::NonZeroI64;

use mcrs_minecraft_environment::WorldClock;
use mcrs_minecraft_predicate::PredicateScratch;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_resource::Id;

use crate::int_provider::IntProvider;
use crate::loot_inputs::LootInputs;
use crate::loot_world::LootWorld;
use crate::number_range::NumberRange;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TimeCheck {
    pub(crate) clock: Option<Id<WorldClock>>,
    pub(crate) period: Option<NonZeroI64>,
    pub(crate) value: NumberRange<IntProvider>,
}

impl TimeCheck {
    pub(crate) fn test<E: Copy, W: LootWorld<E>, R: Random>(
        &self,
        inputs: LootInputs<'_, '_, E, W>,
        random: &mut R,
        scratch: &mut PredicateScratch,
    ) -> bool {
        let ticks = self
            .clock
            .and_then(|clock| inputs.world.total_ticks(clock))
            .unwrap_or(0);
        let ticks = self
            .period
            .map_or(ticks, |period| ticks.wrapping_rem(period.get()));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the low 32 bits of the total ticks"
        )]
        let low = ticks as i32;
        self.value.contains(low, inputs, random, scratch)
    }
}
