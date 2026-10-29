use mcrs_minecraft_command::ErrorText;
use mcrs_minecraft_protocol::text::Text;

/// The errors of a tick count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeError {
    TimeInvalidUnit,
    TickCountTooLow { min: i32, ticks: i32 },
}

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
