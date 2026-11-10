use std::str::FromStr;

use anyhow::bail;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    /// `PERF_KIT_LANG` as the scripts export it; anything else is English.
    #[expect(
        clippy::disallowed_methods,
        reason = "the tester picks the language of the summary once, at the start of run.sh"
    )]
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var("PERF_KIT_LANG") {
            Ok(name) => name.parse().unwrap_or(Self::En),
            Err(_) => Self::En,
        }
    }

    #[must_use]
    pub fn pick(self, en: &'static str, ru: &'static str) -> &'static str {
        match self {
            Self::En => en,
            Self::Ru => ru,
        }
    }
}

impl FromStr for Lang {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        match name {
            "en" => Ok(Self::En),
            "ru" => Ok(Self::Ru),
            other => bail!("unknown language {other}; expected en or ru"),
        }
    }
}
