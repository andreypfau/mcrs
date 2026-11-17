use crate::condition::Condition;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DispatchCase<N> {
    pub(crate) condition: Condition,
    pub(crate) value: N,
}
