use crate::Aabb;

/// The inside shape of a block state as the builder takes it, in block units relative to the
/// block position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InsideBoxes<'a> {
    FullCube,
    Boxes(&'a [Aabb]),
    /// The collision shape for the entity, or the full cube when that shape is empty: powder
    /// snow.
    Collision,
}

/// The inside shape of a block state in the table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InsideShape {
    #[default]
    FullCube,
    Boxes(BoxRange),
    Collision,
}

/// The boxes of an inside shape in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoxRange {
    pub(crate) start: u32,
    pub(crate) end: u32,
}
