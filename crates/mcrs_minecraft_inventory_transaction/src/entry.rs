use crate::Planned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry<I, T> {
    pub slot: I,
    pub before: Option<Planned<T>>,
    pub after: Option<Planned<T>>,
}
