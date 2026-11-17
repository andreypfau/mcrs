use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestSource(pub(crate) u16);

impl fmt::Display for RequestSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
