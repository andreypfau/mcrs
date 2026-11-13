/// Where an element sits in its free space, from 0 to 1 on each axis: 0 is the left or the top.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Alignment {
    pub x: f32,
    pub y: f32,
}

impl Alignment {
    /// The centre on both axes.
    pub const CENTER: Self = Self { x: 0.5, y: 0.5 };
}
