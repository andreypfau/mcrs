use core::error::Error;
use core::fmt;

use crate::Denial;
use crate::Rejected;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Rejected(Rejected),
    Denied(Denial),
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(rejected) => write!(formatter, "rejected: {rejected}"),
            Self::Denied(denial) => write!(
                formatter,
                "denied by source {} with code {}",
                denial.source, denial.code
            ),
        }
    }
}

impl Error for Refusal {}
