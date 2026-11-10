use mcrs_minecraft_core::{CardinalLight, Direction};

/// What a model face multiplies the colour of its vertices by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FaceShade {
    /// The factor of this direction in the shade table of the dimension.
    Direction(Direction),
    /// A factor that is the same in each dimension.
    Factor(f32),
}

impl FaceShade {
    #[must_use]
    pub fn factor(self, table: CardinalLight) -> f32 {
        match self {
            FaceShade::Direction(direction) => table.factor(direction),
            FaceShade::Factor(factor) => factor,
        }
    }
}

/// The value the terrain shaders read to select the shade table.
#[must_use]
pub const fn shader_index(light: CardinalLight) -> u32 {
    match light {
        CardinalLight::Default => 0,
        CardinalLight::Nether => 1,
    }
}
