#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

mod alloc_free;
mod entity;

use mcrs_command::{Argument, Reader, SyntaxError};
use mcrs_minecraft_command::{CommandLevel, NoLookup, Source};

pub const OP: Source<'static, NoLookup> = Source::new(CommandLevel::Gamemasters, &NoLookup);
pub const GUEST: Source<'static, NoLookup> = Source::new(CommandLevel::All, &NoLookup);

/// The value or error of `argument` on `input`, and where the cursor stopped.
pub fn read<'a, T: Argument<Source<'static, NoLookup>>>(
    argument: &T,
    input: &'a str,
    source: &Source<'static, NoLookup>,
) -> (Result<T::Value<'a>, SyntaxError<T::Error>>, usize) {
    let mut reader = Reader::new(input);
    let value = argument.parse(&mut reader, source);
    (value, reader.cursor())
}
