use crate::draws::Draws;

/// A number provider of a block table.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Number {
    Constant(f32),
    Uniform(Box<Number>, Box<Number>),
    Binomial(Box<Number>, Box<Number>),
}

impl Number {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a constant read as an int drops its fraction"
    )]
    pub(crate) fn int(&self, draws: &mut Draws) -> i32 {
        match self {
            Number::Constant(value) => *value as i32,
            Number::Uniform(min, max) => {
                let (min, max) = (min.int(draws), max.int(draws));
                draws.between(min, max)
            }
            Number::Binomial(n, p) => {
                let (n, p) = (n.int(draws), p.float(draws));
                draws.successes(n, p)
            }
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "a binomial count is far below the exact range of f32"
    )]
    pub(crate) fn float(&self, draws: &mut Draws) -> f32 {
        match self {
            Number::Constant(value) => *value,
            Number::Uniform(min, max) => {
                let (min, max) = (min.float(draws), max.float(draws));
                min + draws.unit() * (max - min)
            }
            Number::Binomial(..) => self.int(draws) as f32,
        }
    }
}
