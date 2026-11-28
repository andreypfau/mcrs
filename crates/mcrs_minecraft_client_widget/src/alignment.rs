#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Alignment {
    pub x: f32,
    pub y: f32,
}

impl Alignment {
    pub const CENTER: Self = Self { x: 0.5, y: 0.5 };
}
