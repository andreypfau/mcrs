use core::fmt;
use core::str::FromStr;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    Inline,
    Thread,
}

impl FromStr for Placement {
    type Err = ParsePlacementError;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsePlacementError;

impl fmt::Display for ParsePlacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("expected inline or thread")
    }
}

impl core::error::Error for ParsePlacementError {}
