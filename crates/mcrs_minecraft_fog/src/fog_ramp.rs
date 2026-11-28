#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FogRamp {
    pub start: f32,
    pub end: f32,
}

impl FogRamp {
    pub const NONE: Self = Self {
        start: f32::MAX,
        end: f32::MAX,
    };
}
