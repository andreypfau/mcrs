use core::error::Error;
use core::fmt;

use crate::Reason;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub op: usize,
    pub reason: Reason,
}

impl fmt::Display for Rejected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "operation {}: {}", self.op, self.reason)
    }
}

impl Error for Rejected {}
