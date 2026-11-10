use mcrs_command::{Argument, Reader, SyntaxError};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_snbt::{SnbtError, check_compound, read_compound};

/// `minecraft:nbt_compound_tag`: an SNBT compound (SNBT-ARG-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NbtCompoundArgument;

/// The text of a compound that the argument checked. The value borrows the command line, so a
/// parse allocates nothing; [`Self::read`] builds the compound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NbtCompoundText<'a> {
    input: &'a str,
    start: usize,
}

impl NbtCompoundText<'_> {
    /// The compound. The parse checked the text, so the read fails only on input that a check
    /// accepts and a read refuses.
    pub fn read(&self) -> Result<NbtCompound, SnbtError> {
        read_compound(self.input, self.start).map(|(compound, _)| compound)
    }
}

impl<S> Argument<S> for NbtCompoundArgument {
    type Value<'a> = NbtCompoundText<'a>;
    type Error = SnbtError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<NbtCompoundText<'a>, SyntaxError<SnbtError>> {
        let start = reader.cursor();
        let end = check_compound(reader.input(), start)
            .map_err(|error| SyntaxError::argument(error, error.position))?;
        reader.set_cursor(end);
        Ok(NbtCompoundText {
            input: reader.input(),
            start,
        })
    }
}
