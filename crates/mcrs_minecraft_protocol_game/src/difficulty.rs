use mcrs_minecraft_protocol::{Decode, Encode};

pub use mcrs_minecraft_entity::Difficulty as WorldDifficulty;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Encode, Decode)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl From<WorldDifficulty> for Difficulty {
    fn from(difficulty: WorldDifficulty) -> Self {
        match difficulty {
            WorldDifficulty::Peaceful => Self::Peaceful,
            WorldDifficulty::Easy => Self::Easy,
            WorldDifficulty::Normal => Self::Normal,
            WorldDifficulty::Hard => Self::Hard,
        }
    }
}

impl From<Difficulty> for WorldDifficulty {
    fn from(difficulty: Difficulty) -> Self {
        match difficulty {
            Difficulty::Peaceful => Self::Peaceful,
            Difficulty::Easy => Self::Easy,
            Difficulty::Normal => Self::Normal,
            Difficulty::Hard => Self::Hard,
        }
    }
}
