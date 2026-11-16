use crate::condition::Condition;

/// One case of `number_dispatcher` (LOOT-NUM-014).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DispatchCase<N> {
    pub(crate) condition: Condition,
    pub(crate) value: N,
}
