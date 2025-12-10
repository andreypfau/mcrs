use std::io::Write;

use anyhow::bail;
use derive_more::{From, Into};

use crate::{Decode, Encode};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default, Encode, Decode)]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    #[inline]
    pub fn is_block_placing_restricted(&self) -> bool {
        self == &GameMode::Adventure || self == &GameMode::Spectator
    }
}

/// An optional [`GameMode`] with `None` encoded as `-1`. Isomorphic to
/// `Option<GameMode>`.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, From, Into)]
pub struct OptGameMode(pub Option<GameMode>);

impl Encode for OptGameMode {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        match self.0 {
            Some(gm) => (gm as i8).encode(w),
            None => (-1i8).encode(w),
        }
    }
}

impl Decode<'_> for OptGameMode {
    fn decode(r: &mut &'_ [u8]) -> anyhow::Result<Self> {
        Ok(Self(match i8::decode(r)? {
            -1 => None,
            0 => Some(GameMode::Survival),
            1 => Some(GameMode::Creative),
            2 => Some(GameMode::Adventure),
            3 => Some(GameMode::Spectator),
            other => bail!("invalid game mode byte of {other}"),
        }))
    }
}
