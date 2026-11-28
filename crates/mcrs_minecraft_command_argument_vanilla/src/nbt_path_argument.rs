use mcrs_command::{Argument, Reader, SyntaxError};
use mcrs_minecraft_nbt_path::{NbtPath, NbtPathError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NbtPathArgument;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NbtPathValue {
    pub text: String,
    pub path: NbtPath,
}

impl<S> Argument<S> for NbtPathArgument {
    type Value<'a> = NbtPathValue;
    type Error = NbtPathError;

    fn parse(
        &self,
        reader: &mut Reader<'_>,
        _: &S,
    ) -> Result<NbtPathValue, SyntaxError<NbtPathError>> {
        let start = reader.cursor();
        let span = reader.read_while(|character| character != ' ');
        let text = span.text(reader.input());
        let path: NbtPath = text.parse().map_err(|error: NbtPathError| {
            SyntaxError::argument(error, start.saturating_add(error.position()))
        })?;
        Ok(NbtPathValue {
            text: String::from(text),
            path,
        })
    }
}
