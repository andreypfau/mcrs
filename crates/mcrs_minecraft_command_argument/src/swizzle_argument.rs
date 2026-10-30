use crate::coordinates_error::CoordinatesError;
use glam::BVec3;
use mcrs_command::{Argument, Reader, SEPARATOR, SyntaxError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwizzleArgument;

impl<S> Argument<S> for SwizzleArgument {
    type Value<'a> = BVec3;
    type Error = CoordinatesError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<BVec3, SyntaxError<CoordinatesError>> {
        let mut axes = BVec3::FALSE;
        let invalid = |reader: &Reader<'_>| {
            SyntaxError::argument(CoordinatesError::SwizzleInvalid, reader.cursor())
        };
        while let Some(c) = reader.peek().filter(|&c| c != SEPARATOR) {
            reader.skip();
            let axis = match c {
                'x' => &mut axes.x,
                'y' => &mut axes.y,
                'z' => &mut axes.z,
                _ => return Err(invalid(reader)),
            };
            if *axis {
                return Err(invalid(reader));
            }
            *axis = true;
        }
        if axes == BVec3::FALSE {
            return Err(invalid(reader));
        }
        Ok(axes)
    }
}
