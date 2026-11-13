use core::error::Error;
use core::fmt;

/// Why the kernel gives no request for a click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClickError {
    /// The clicked menu index is not an entry of the menu. The server does nothing and sends
    /// nothing.
    NoSuchEntry,
    /// A slot of the player that the click names does not load.
    NoSuchSlot,
    /// The click gives more operations than a request can hold. The server applies nothing and
    /// corrects the client as after a rejection.
    TooManyOps,
}

impl fmt::Display for ClickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NoSuchEntry => "the menu has no such entry",
            Self::NoSuchSlot => "the player has no such slot",
            Self::TooManyOps => "the click gives too many operations",
        })
    }
}

impl Error for ClickError {}
