use mcrs_command::{Argument, Reader, SyntaxError};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_snbt::{SnbtError, check_compound, read_compound};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NbtCompoundArgument;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NbtCompoundText<'a> {
    input: &'a str,
    start: usize,
}

impl NbtCompoundText<'_> {
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
