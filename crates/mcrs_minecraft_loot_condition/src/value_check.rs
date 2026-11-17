use crate::number_range::NumberRange;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValueCheck<P> {
    pub(crate) value: P,
    pub(crate) test: NumberRange<P>,
}
