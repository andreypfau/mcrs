use crate::Filter;
use crate::Planned;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loaded<T> {
    pub stack: Option<Planned<T>>,
    pub limit: u8,
    pub take: Filter,
}
