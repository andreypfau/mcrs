use crate::closed::Closed;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    Ignored,
    Handled,
    Closed(Closed),
    Page(isize),
    Copy(String),
}
