use crate::{Decode, Encode};

/// The part of an entity that a look or a position is taken from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub enum LookAnchor {
    /// The position of the entity.
    #[default]
    Feet,
    /// The position of the entity raised by its eye height.
    Eyes,
}
