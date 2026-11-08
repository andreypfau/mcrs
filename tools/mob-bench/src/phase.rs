use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

/// One measured state of the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// The world with the pen and the forceloaded areas, and no pig.
    Idle,
    /// The pigs spread over the open area.
    Roam,
    /// The pigs in the pen.
    Crowd,
    /// The pigs in the pen while a group in its middle takes damage.
    Stimulus,
}

impl FromStr for Phase {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "idle" => Phase::Idle,
            "roam" => Phase::Roam,
            "crowd" => Phase::Crowd,
            "stimulus" => Phase::Stimulus,
            other => bail!("unknown phase {other}"),
        })
    }
}
