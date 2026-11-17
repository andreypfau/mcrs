use crate::RequestId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Denial {
    pub source: RequestId,
    pub code: u16,
}
