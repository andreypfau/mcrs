use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

use crate::scene::Ai;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Idle,
    RoamStill,
    RoamStir,
    CrowdStill,
    CrowdStir,
    Stimulus,
    CostStill,
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
