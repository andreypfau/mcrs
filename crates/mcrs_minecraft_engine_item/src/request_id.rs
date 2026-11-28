use crate::RequestSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestId {
    pub source: RequestSource,
    pub serial: u32,
}
