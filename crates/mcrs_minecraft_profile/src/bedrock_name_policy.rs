use std::collections::HashSet;
use std::fmt;
use std::hash::BuildHasher;

use crate::{PlayerName, player_name};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BedrockNamePolicy {
    pub prefix: Option<char>,
    pub space_replacement: Option<char>,
    pub max_length: usize,
}

impl Default for BedrockNamePolicy {
    fn default() -> Self {
        Self {
            prefix: Some('.'),
            space_replacement: Some('_'),
            max_length: 16,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BedrockNameError {
    InvalidPolicy,
    Empty,
    NoFreeName,
}

impl fmt::Display for BedrockNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPolicy => "the name policy uses a character a player name cannot hold",
            Self::Empty => "the gamertag gives an empty player name",
            Self::NoFreeName => "no free player name fits in the maximum length",
        })
    }
}

impl std::error::Error for BedrockNameError {}

impl BedrockNamePolicy {
    pub fn java_name<S: BuildHasher>(
        &self,
        gamertag: &str,
        online: &HashSet<PlayerName, S>,
    ) -> Result<PlayerName, BedrockNameError> {
        let legal = |c: Option<char>| c.is_none_or(|c| c.is_ascii_graphic());
        if !legal(self.prefix) || !legal(self.space_replacement) {
            return Err(BedrockNameError::InvalidPolicy);
        }
        let max_length = self.max_length.min(16);
        let mut full = String::new();
        full.extend(self.prefix);
        for c in gamertag.chars() {
            if c == ' ' {
                full.extend(self.space_replacement);
            } else if c.is_ascii_graphic() {
                full.push(c);
            }
        }
        full.truncate(max_length);
        if full.is_empty() {
            return Err(BedrockNameError::Empty);
        }
        let name = |text: &str| player_name::player_name(text).map_err(|_| BedrockNameError::Empty);
        let base = name(&full)?;
        if !online.contains(&base) {
            return Ok(base);
        }
        for suffix in 1..=usize::MAX {
            let suffix = suffix.to_string();
            let keep = max_length.saturating_sub(suffix.len());
            if keep == 0 {
                return Err(BedrockNameError::NoFreeName);
            }
            full.truncate(keep);
            let candidate = name(&format!("{full}{suffix}"))?;
            if !online.contains(&candidate) {
                return Ok(candidate);
            }
        }
        Err(BedrockNameError::NoFreeName)
    }
}
