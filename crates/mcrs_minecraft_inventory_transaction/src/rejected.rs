use core::error::Error;
use core::fmt;

use crate::Reason;

/// The result of a request that does not apply: the first operation with a false condition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rejected {
    /// The index of the operation in the request, from 0.
    pub op: usize,
    pub reason: Reason,
}

impl fmt::Display for Rejected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "operation {}: {}", self.op, self.reason)
    }
}

impl Error for Rejected {}
