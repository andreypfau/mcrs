use crate::int_provider::IntProvider;
use crate::number_range::NumberRange;

/// One entry of the `scores` of `entity_scores` (LOOT-CND-012).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ObjectiveTest {
    pub(crate) objective: Box<str>,
    pub(crate) range: NumberRange<IntProvider>,
}
