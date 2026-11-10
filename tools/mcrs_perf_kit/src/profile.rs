use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Quick,
    Full,
}

impl FromStr for Profile {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        match name {
            "quick" => Ok(Self::Quick),
            "full" => Ok(Self::Full),
            other => bail!("unknown profile {other}; expected quick or full"),
        }
    }
}
