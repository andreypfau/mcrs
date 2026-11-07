use mcrs_minecraft_core::Direction;

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
    pub fn factor(self, table: ShadeTable) -> f32 {
        match self {
            FaceShade::Direction(direction) => table.factor(direction),
            FaceShade::Factor(factor) => factor,
        }
    }
}

/// The directional shade factors of a dimension type, one table per value of its
/// `cardinal_light`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShadeTable {
    #[default]
    Default,
    Nether,
}

impl ShadeTable {
    #[must_use]
    pub const fn factor(self, direction: Direction) -> f32 {
        match (self, direction) {
            (ShadeTable::Default, Direction::Down) => 0.5,
            (ShadeTable::Default, Direction::Up) => 1.0,
            (ShadeTable::Nether, Direction::Down | Direction::Up) => 0.9,
            (_, Direction::North | Direction::South) => 0.8,
            (_, Direction::West | Direction::East) => 0.6,
        }
    }

    /// The value the terrain shaders read to select the table.
    #[must_use]
    pub const fn shader_index(self) -> u32 {
        match self {
            ShadeTable::Default => 0,
            ShadeTable::Nether => 1,
        }
    }
}
