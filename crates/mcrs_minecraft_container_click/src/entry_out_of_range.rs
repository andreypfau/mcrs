use core::error::Error;
use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EntryOutOfRange;

impl fmt::Display for EntryOutOfRange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the clicked entry is past the largest slot index")
    }
}

impl Error for EntryOutOfRange {}
