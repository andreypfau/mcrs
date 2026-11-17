use crate::int_provider::IntProvider;
use crate::number_range::NumberRange;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ObjectiveTest {
    pub(crate) objective: Box<str>,
    pub(crate) range: NumberRange<IntProvider>,
}
