use crate::{Decode, Encode};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub enum Direction {
    /// -Y
    Down,
    /// +Y
    Up,
    /// -Z
    North,
    /// +Z
    South,
    /// -X
    West,
    /// +X
    East,
}
