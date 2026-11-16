use core::error::Error;
use core::fmt;

use crate::Denial;
use crate::Rejected;

/// Why a request changed nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The planner rejected an operation.
    Rejected(Rejected),
    /// An observer of [`RequestCheck`](crate::RequestCheck) denied the request.
    Denied(Denial),
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rejected) => write!(formatter, "rejected: {rejected}"),
            Self::Denied(denial) => write!(
                formatter,
                "denied by source {} with code {}",
                denial.source.0, denial.code
            ),
        }
    }
}

impl Error for Refusal {}
