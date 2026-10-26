#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod alloc_free;
mod arguments;
mod block;
mod entity;
mod tree;

use mcrs_command::{Argument, Reader};
use mcrs_minecraft_command::{CommandLevel, CommandSyntaxError, NoLookup, Source};

pub const OP: Source<'static, NoLookup> = Source::new(CommandLevel::Gamemasters, &NoLookup);
pub const GUEST: Source<'static, NoLookup> = Source::new(CommandLevel::All, &NoLookup);

/// The value or error of `argument` on `input`, and where the cursor stopped.
pub fn read<'a, T>(
    argument: &T,
    input: &'a str,
    source: &Source<'static, NoLookup>,
) -> (Result<T::Value<'a>, CommandSyntaxError>, usize)
where
    T: Argument<Source<'static, NoLookup>, Error = mcrs_minecraft_command::ArgumentError>,
{
    let mut reader = Reader::new(input);
    let value = argument.parse(&mut reader, source);
    (value, reader.cursor())
}
