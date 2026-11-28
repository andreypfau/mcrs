use crate::RequestSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Denial {
    pub source: RequestSource,
    pub code: u16,
}
