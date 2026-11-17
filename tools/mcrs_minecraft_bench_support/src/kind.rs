use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

use crate::process::Stop;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Mcrs,
    Vanilla,
}

impl FromStr for Kind {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        Ok(match name {
            "mcrs" => Kind::Mcrs,
            "vanilla" => Kind::Vanilla,
            other => bail!("unknown server {other}"),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TickMetric {
    TickQuery,
    McrsTrace,
}

impl Kind {
    #[must_use]
    pub fn tick_metric(self) -> TickMetric {
        match self {
            Kind::Mcrs => TickMetric::McrsTrace,
            Kind::Vanilla => TickMetric::TickQuery,
        }
    }

    #[must_use]
    pub fn stop(self) -> Stop {
        match self {
            Kind::Mcrs => Stop::Interrupt,
            Kind::Vanilla => Stop::Console,
        }
    }
}
