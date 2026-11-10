use crate::Planned;

/// One slot that an applied request changed, with the stack before and the stack after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<I, T> {
    pub slot: I,
    pub before: Option<Planned<T>>,
    pub after: Option<Planned<T>>,
}
