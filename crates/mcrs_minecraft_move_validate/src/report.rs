use crate::Finding;

/// A finding for the layer above, with the client tick of the packet, numbered from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub finding: Finding,
    pub client_tick: u64,
}
