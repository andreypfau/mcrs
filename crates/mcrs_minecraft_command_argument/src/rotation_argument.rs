use crate::coordinate_reader::{Centre, read_world, separator};
use crate::coordinates_error::CoordinatesError;
use crate::rotation_value::RotationValue;
use mcrs_command::{Argument, Reader, SyntaxError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RotationArgument;

impl<S> Argument<S> for RotationArgument {
    type Value<'a> = RotationValue;
    type Error = CoordinatesError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<RotationValue, SyntaxError<CoordinatesError>> {
        let start = reader.cursor();
        if reader.at_end() {
            return Err(SyntaxError::argument(
                CoordinatesError::RotationIncomplete,
                reader.cursor(),
            ));
        }
        let yaw = read_world(reader, Centre::Exact)?;
        separator(reader, start, CoordinatesError::RotationIncomplete)?;
        let pitch = read_world(reader, Centre::Exact)?;
        Ok(RotationValue { yaw, pitch })
    }
}
