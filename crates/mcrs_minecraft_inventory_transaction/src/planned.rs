#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Planned<T> {
    pub stack: T,
    pub kind: u16,
    pub count: u8,
    pub max: u8,
}
