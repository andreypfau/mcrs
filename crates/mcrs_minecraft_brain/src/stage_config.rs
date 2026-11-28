use core::error::Error;
use core::fmt;

use crate::AiDelay;
use crate::WorldSeed;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StageConfig {
    seed: WorldSeed,
    max_lag: u8,
    delay: AiDelay,
    wake: WakeMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WakeMode {
    Sleep,
    Poll,
}

impl StageConfig {
    pub const DEFAULT_MAX_LAG: u8 = 2;

    #[must_use]
    pub const fn new(seed: WorldSeed) -> Self {
        Self {
            seed,
            max_lag: Self::DEFAULT_MAX_LAG,
            delay: AiDelay::AtLeast(1),
            wake: WakeMode::Sleep,
        }
    }

    pub fn with_delay(self, max_lag: u8, delay: AiDelay) -> Result<Self, ConfigError> {
        if max_lag == 0 {
            return Err(ConfigError::ZeroMaxLag);
        }
        if delay.ticks() == 0 || delay.ticks() > max_lag {
            return Err(ConfigError::DelayOutsideLag);
        }
        Ok(Self {
            max_lag,
            delay,
            ..self
        })
    }

    #[must_use]
    pub const fn with_wake(self, wake: WakeMode) -> Self {
        Self { wake, ..self }
    }

    #[must_use]
    pub const fn seed(&self) -> WorldSeed {
        self.seed
    }

    #[must_use]
    pub const fn max_lag(&self) -> u8 {
        self.max_lag
    }

    #[must_use]
    pub const fn delay(&self) -> AiDelay {
        self.delay
    }

    #[must_use]
    pub const fn wake(&self) -> WakeMode {
        self.wake
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigError {
    ZeroMaxLag,
    DelayOutsideLag,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ZeroMaxLag => "max_lag must be 1 or more",
            Self::DelayOutsideLag => "the AI delay must be from 1 to max_lag",
        })
    }
}

impl Error for ConfigError {}
