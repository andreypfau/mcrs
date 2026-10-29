use core::fmt;
use mcrs_minecraft_command::ErrorText;
use mcrs_minecraft_protocol::text::Text;

/// The errors of a tick count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TimeError {
    /// The unit after the number is none of `d`, `s` and `t`.
    TimeInvalidUnit,
    /// The tick count is below the minimum of the argument.
    TickCountTooLow { min: i32, ticks: i32 },
}

impl fmt::Display for TimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeError::TimeInvalidUnit => f.write_str("invalid time unit"),
            TimeError::TickCountTooLow { min, ticks } => {
                write!(f, "tick count {ticks} is below {min}")
            }
        }
    }
}

impl core::error::Error for TimeError {}

impl ErrorText for TimeError {
    fn text(&self, _: &str) -> Text {
        match *self {
            TimeError::TimeInvalidUnit => Text::translate("argument.time.invalid_unit", vec![]),
            TimeError::TickCountTooLow { min, ticks } => Text::translate_args(
                "argument.time.tick_count_too_low",
                vec![min.into(), ticks.into()],
            ),
        }
    }
}
