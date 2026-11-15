/// The answer of a take filter or of a place filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Permits,
    Refuses,
}
