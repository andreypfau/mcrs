use core::fmt;
use core::str::FromStr;

/// Where the far end of a [`Link`](crate::Link) runs.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    /// On the thread of the near end, when the near end takes in the outputs. The stage then
    /// runs until it owes nothing, so the outputs do not depend on thread timing.
    Inline,
    /// On an OS thread of its own, which runs whenever an input or a task outcome arrives.
    Thread,
}

impl FromStr for Placement {
    type Err = ParsePlacementError;

    /// Reads `inline` or `thread`, in any case.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        if name.eq_ignore_ascii_case("inline") {
            Ok(Self::Inline)
        } else if name.eq_ignore_ascii_case("thread") {
            Ok(Self::Thread)
        } else {
            Err(ParsePlacementError)
        }
    }
}

/// A name that is neither `inline` nor `thread`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsePlacementError;

impl fmt::Display for ParsePlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected inline or thread")
    }
}

impl core::error::Error for ParsePlacementError {}
