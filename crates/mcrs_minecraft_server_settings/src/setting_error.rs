use alloc::string::String;
use core::fmt;

/// A setting whose value does not read; the setting keeps its default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingError {
    /// The name of the setting, without the `MCRS_` prefix.
    pub name: &'static str,
    /// The value as it was set.
    pub value: String,
    /// What the setting takes.
    pub expected: &'static str,
}

impl fmt::Display for SettingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MCRS_{}={:?} is not {}; keeping the default",
            self.name, self.value, self.expected
        )
    }
}

impl core::error::Error for SettingError {}
