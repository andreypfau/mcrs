use std::io::Write;

use anyhow::bail;
use derive_more::{From, Into};

use crate::{Decode, Encode, VarInt};

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

/// An optional [`GameMode`] on the wire as a VarInt: `0` is `None`, otherwise
/// the mode id plus one.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, From, Into)]
pub struct OptGameMode(pub Option<GameMode>);

impl Encode for OptGameMode {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        VarInt(self.0.map_or(0, |gm| gm as i32 + 1)).encode(w)
    }
}

impl Decode<'_> for OptGameMode {
    fn decode(r: &mut &'_ [u8]) -> anyhow::Result<Self> {
        Ok(Self(match VarInt::decode(r)?.0 {
            0 => None,
            1 => Some(GameMode::Survival),
            2 => Some(GameMode::Creative),
            3 => Some(GameMode::Adventure),
            4 => Some(GameMode::Spectator),
            other => bail!("invalid optional game mode of {other}"),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_game_mode_is_a_shifted_var_int() {
        for (mode, byte) in [
            (None, 0u8),
            (Some(GameMode::Survival), 1),
            (Some(GameMode::Creative), 2),
            (Some(GameMode::Spectator), 4),
        ] {
            let mut out = Vec::new();
            OptGameMode(mode).encode(&mut out).unwrap();
            assert_eq!(out, [byte]);
            assert_eq!(OptGameMode::decode(&mut &out[..]).unwrap().0, mode);
        }
    }
}
