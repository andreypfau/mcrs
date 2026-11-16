use crate::number_range::NumberRange;

/// The fields of `int_value_check` and `float_value_check` (LOOT-CND-016, LOOT-CND-017).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValueCheck<P> {
    pub(crate) value: P,
    pub(crate) test: NumberRange<P>,
}
