use core::fmt;
use core::str::FromStr;
use std::io::Write;

use derive_more::{From, Into};
use mcrs_minecraft_wire::{Decode, Encode, Result, VarInt, bail};

#[derive(
    Copy, Clone, PartialEq, Eq, Debug, Default, Encode, Decode, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub const ALL: [GameMode; 4] = [
        GameMode::Survival,
        GameMode::Creative,
        GameMode::Adventure,
        GameMode::Spectator,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            GameMode::Survival => "survival",
            GameMode::Creative => "creative",
            GameMode::Adventure => "adventure",
            GameMode::Spectator => "spectator",
        }
    }

    pub const fn translation_key(self) -> &'static str {
        match self {
            GameMode::Survival => "gameMode.survival",
            GameMode::Creative => "gameMode.creative",
            GameMode::Adventure => "gameMode.adventure",
            GameMode::Spectator => "gameMode.spectator",
        }
    }

    #[inline]
    pub const fn is_block_placing_restricted(self) -> bool {
        matches!(self, GameMode::Adventure | GameMode::Spectator)
    }
}

impl fmt::Display for GameMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A name that no [`GameMode`] has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownGameMode;

impl fmt::Display for UnknownGameMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown game mode")
    }
}

impl core::error::Error for UnknownGameMode {}

impl FromStr for GameMode {
    type Err = UnknownGameMode;

    fn from_str(name: &str) -> core::result::Result<Self, UnknownGameMode> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name() == name)
            .ok_or(UnknownGameMode)
    }
}

/// An optional [`GameMode`] on the wire as a VarInt: `0` is `None`, otherwise
/// the mode id plus one.
#[derive(Copy, Clone, PartialEq, Eq, Default, Debug, From, Into)]
pub struct OptGameMode(pub Option<GameMode>);

impl Encode for OptGameMode {
    fn encode(&self, w: impl Write) -> Result<()> {
        VarInt(self.0.map_or(0, |gm| gm as i32 + 1)).encode(w)
    }
}

impl Decode<'_> for OptGameMode {
    fn decode(r: &mut &'_ [u8]) -> Result<Self> {
        let id = VarInt::decode(r)?.0;
        if id == 0 {
            return Ok(Self(None));
        }
        let mode = usize::try_from(id.wrapping_sub(1))
            .ok()
            .and_then(|index| GameMode::ALL.get(index));
        match mode {
            Some(&mode) => Ok(Self(Some(mode))),
            None => bail!("invalid optional game mode of {id}"),
        }
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
