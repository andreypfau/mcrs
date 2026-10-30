use crate::{OP, read};
use mcrs_command::{LibraryError, SyntaxError};
use mcrs_minecraft_command_argument::{TimeArgument, TimeError};

#[test]
fn time_reads_a_tick_count_in_its_unit() {
    let at_least_zero = TimeArgument { min: 0 };
    let any = TimeArgument { min: i32::MIN };
    let low = |ticks, cursor| {
        Err(SyntaxError::argument(
            TimeError::TickCountTooLow { min: 0, ticks },
            cursor,
        ))
    };
    let cases = [
        (at_least_zero, "1d", Ok(24_000)),
        (at_least_zero, "1.5s", Ok(30)),
        (at_least_zero, "0.1d", Ok(2_400)),
        (at_least_zero, "0.5", Ok(1)),
        (any, "-0.5", Ok(0)),
        (at_least_zero, "100000d", Ok(i32::MAX)),
        (
            at_least_zero,
            "1D",
            Err(SyntaxError::argument(TimeError::TimeInvalidUnit, 2)),
        ),
        (
            at_least_zero,
            "1e3",
            Err(SyntaxError::argument(TimeError::TimeInvalidUnit, 3)),
        ),
        (at_least_zero, "-1", low(-1, 2)),
        (
            at_least_zero,
            "d",
            Err(SyntaxError::library(LibraryError::ExpectedFloat, 0)),
        ),
    ];
    for (argument, input, expected) in cases {
        assert_eq!(read(&argument, input, &OP).0, expected, "{input}");
    }
}
