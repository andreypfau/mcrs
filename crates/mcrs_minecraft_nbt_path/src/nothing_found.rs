use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NothingFound {
    pub end: usize,
}

impl fmt::Display for NothingFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the path node that ends at {} selects nothing", self.end)
    }
}

impl core::error::Error for NothingFound {}
