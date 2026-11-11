use std::collections::HashSet;
use std::fmt;
use std::hash::BuildHasher;

use crate::PlayerName;

/// How a gamertag becomes a legal Java name. The name is never an identity key: a
/// gamertag can change, and a cut can give two players one name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BedrockNamePolicy {
    pub prefix: Option<char>,
    /// `None` drops spaces.
    pub space_replacement: Option<char>,
    /// At most 16.
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
    /// The prefix or the space replacement is not printable ASCII without a space.
    InvalidPolicy,
    /// No character of the gamertag survives, the policy has no prefix, or `max_length` is 0.
    Empty,
    /// The suffix that makes the name unique leaves no character of the name in `max_length`.
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
    /// `name` when no online player holds it, else the first free `name1`, `name2`, ….
    /// A suffix replaces the tail of the name, so the length stays within `max_length`.
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
        let name = |text: &str| crate::player_name(text).map_err(|_| BedrockNameError::Empty);
        let base = name(&full)?;
        if !online.contains(&base) {
            return Ok(base);
        }
        // One cut for every suffix keeps the candidates distinct, so one of
        // online.len() + 1 of them is free.
        let last = online.len() + 1;
        let keep = max_length.saturating_sub(last.to_string().len());
        if keep == 0 {
            return Err(BedrockNameError::NoFreeName);
        }
        full.truncate(keep);
        for suffix in 1..=last {
            let candidate = name(&format!("{full}{suffix}"))?;
            if !online.contains(&candidate) {
                return Ok(candidate);
            }
        }
        Err(BedrockNameError::NoFreeName)
    }
}
