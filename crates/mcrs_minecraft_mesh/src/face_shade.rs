use mcrs_minecraft_core::{CardinalLight, Direction};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FaceShade {
    Direction(Direction),
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

#[must_use]
pub const fn shader_index(light: CardinalLight) -> u32 {
    match light {
        CardinalLight::Default => 0,
        CardinalLight::Nether => 1,
    }
}
