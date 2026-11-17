use crate::Finding;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub finding: Finding,
    pub client_tick: u64,
}
