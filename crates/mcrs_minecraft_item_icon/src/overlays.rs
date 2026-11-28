#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Overlays {
    None,
    Stack,
    Capped,
}
