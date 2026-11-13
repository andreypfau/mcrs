/// The margins of a cell around its child in GUI units, each ≥ 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Padding {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
