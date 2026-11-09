use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

use crate::scene::Ai;

/// One measured state of the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// The world with the pen and the forceloaded areas, and no pig.
    Idle,
    /// The pigs spread over the open area, with no stir: they stand and look around.
    RoamStill,
    /// The pigs spread over the open area, stirred.
    RoamStir,
    /// The pigs in the pen, with no stir: only the pushes move them.
    CrowdStill,
    /// The pigs in the pen, stirred.
    CrowdStir,
    /// The pigs in the pen, stirred, while a rotating group of them takes panic hits.
    Stimulus,
    /// Pigs with `NoAI:1b` in the pen, with no stir: the base of the stir cost.
    CostStill,
    /// Pigs with `NoAI:1b` in the pen, stirred: the cost of the stir lines alone.
    CostStir,
}

impl Phase {
    #[must_use]
    pub fn stirred(self) -> bool {
        matches!(
            self,
            Phase::RoamStir | Phase::CrowdStir | Phase::Stimulus | Phase::CostStir
        )
    }

    #[must_use]
    pub fn roams(self) -> bool {
        matches!(self, Phase::RoamStill | Phase::RoamStir)
    }

    #[must_use]
    pub fn ai(self) -> Ai {
        match self {
            Phase::CostStill | Phase::CostStir => Ai::Off,
            _ => Ai::On,
        }
    }
}

impl FromStr for Phase {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "idle" => Phase::Idle,
            "roam-still" => Phase::RoamStill,
            "roam-stir" => Phase::RoamStir,
            "crowd-still" => Phase::CrowdStill,
            "crowd-stir" => Phase::CrowdStir,
            "stimulus" => Phase::Stimulus,
            "cost-still" => Phase::CostStill,
            "cost-stir" => Phase::CostStir,
            other => bail!("unknown phase {other}"),
        })
    }
}
