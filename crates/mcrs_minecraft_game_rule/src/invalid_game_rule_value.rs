use core::error::Error;
use core::fmt;

use crate::AnyGameRule;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidGameRuleValue {
    pub rule: AnyGameRule,
    pub value: i32,
}

impl fmt::Display for InvalidGameRuleValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "value {} is outside the type and bounds of game rule {}",
            self.value,
            self.rule.location()
        )
    }
}

impl Error for InvalidGameRuleValue {}
