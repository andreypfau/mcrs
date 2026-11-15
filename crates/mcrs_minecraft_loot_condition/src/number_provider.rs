use mcrs_minecraft_predicate::LocationView;
use mcrs_minecraft_random::Random;

use crate::context_params::ContextParams;
use crate::draw;
use crate::loot_context::LootContext;

/// A baked context number provider. Its evaluation takes the draws of LOOT-NUM-002 in order.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberProvider(pub(crate) Provider);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Provider {
    Constant(f32),
    Uniform(Box<Provider>, Box<Provider>),
    Binomial(Box<Provider>, Box<Provider>),
}

impl NumberProvider {
    /// The provider `constant` (LOOT-NUM-003).
    #[must_use]
    pub const fn constant(value: f32) -> Self {
        Self(Provider::Constant(value))
    }

    /// The int read (LOOT-NUM-001): an evaluation error gives 0.
    pub fn read_int<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
    ) -> i32 {
        self.0.int(context, world, random).unwrap_or(0)
    }

    /// The float read (LOOT-NUM-001): an evaluation error, a NaN and an infinity give 0.
    pub fn read_float<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
    ) -> f32 {
        self.0
            .float(context, world, random)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
    }

    /// The parameters that the provider reads.
    #[must_use]
    pub fn reads(&self) -> ContextParams {
        self.0.reads()
    }
}

impl Provider {
    /// The int value; `None` for an evaluation error.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a constant read as an int drops its fraction"
    )]
    fn int<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
    ) -> Option<i32> {
        match self {
            Provider::Constant(value) => Some(*value as i32),
            Provider::Uniform(min, max) => {
                let min = min.int(context, world, random)?;
                let max = max.int(context, world, random)?;
                draw::between(random, min, max)
            }
            Provider::Binomial(n, p) => {
                let trials = n.int(context, world, random)?;
                let chance = p.float(context, world, random)?;
                if !chance.is_finite() {
                    return None;
                }
                Some(draw::binomial(random, trials, chance))
            }
        }
    }

    /// The float value; `None` for an evaluation error.
    #[expect(
        clippy::cast_precision_loss,
        reason = "a binomial count is far below the exact range of f32"
    )]
    fn float<E, W: LocationView, R: Random>(
        &self,
        context: &LootContext<'_, E>,
        world: &W,
        random: &mut R,
    ) -> Option<f32> {
        match self {
            Provider::Constant(value) => Some(*value),
            Provider::Uniform(min, max) => {
                let min = min.float(context, world, random)?;
                let max = max.float(context, world, random)?;
                if min >= max {
                    return Some(min);
                }
                Some(random.next_f32() * (max - min) + min)
            }
            Provider::Binomial(..) => self.int(context, world, random).map(|count| count as f32),
        }
    }

    fn reads(&self) -> ContextParams {
        match self {
            Provider::Constant(_) => ContextParams::EMPTY,
            Provider::Uniform(first, second) | Provider::Binomial(first, second) => {
                first.reads().union(second.reads())
            }
        }
    }
}
