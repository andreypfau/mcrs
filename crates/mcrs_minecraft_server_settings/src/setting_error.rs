use alloc::string::String;
use core::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingError {
    pub name: &'static str,
    pub value: String,
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
