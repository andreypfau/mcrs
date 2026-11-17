#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WeightedEntry<N> {
    pub(crate) weight: i32,
    pub(crate) data: N,
}
