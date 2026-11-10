use std::fmt;
use std::str::FromStr;

use anyhow::bail;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Mob,
    Client,
    ServerStart,
    Worldgen,
}

impl Section {
    pub const ALL: [Self; 4] = [Self::Mob, Self::Client, Self::ServerStart, Self::Worldgen];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Mob => "mob",
            Self::Client => "client",
            Self::ServerStart => "server",
            Self::Worldgen => "worldgen",
        }
    }
}

impl FromStr for Section {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        match Self::ALL.into_iter().find(|section| section.name() == name) {
            Some(section) => Ok(section),
            None => bail!("unknown section {name}; expected mob, client, server or worldgen"),
        }
    }
}

impl fmt::Display for Section {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}
