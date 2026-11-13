/// A fog that is 0 at `start` blocks or nearer, 1 at `end` blocks or farther, and linear
/// between.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FogRamp {
    pub start: f32,
    pub end: f32,
}

impl FogRamp {
    /// The ramp that is 0 at each distance.
    pub const NONE: Self = Self {
        start: f32::MAX,
        end: f32::MAX,
    };
}
