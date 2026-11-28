use crate::Aabb;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InsideBoxes<'a> {
    FullCube,
    Boxes(&'a [Aabb]),
    Collision,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InsideShape {
    #[default]
    FullCube,
    Boxes(BoxRange),
    Collision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoxRange {
    pub(crate) start: u32,
    pub(crate) end: u32,
}
