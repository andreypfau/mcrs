use crate::coordinate_reader::{Completeness, read_block, read_coordinates, suggest_coordinates};
use crate::coordinates::Coordinates;
use crate::coordinates_error::CoordinatesError;
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};

/// `minecraft:block_pos`. The value is the coordinates; the block position
/// is [`crate::block_floor()`] of their point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockPosArgument;

impl BlockPosArgument {
    fn read(
        reader: &mut Reader<'_>,
        completeness: Completeness,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
        read_coordinates(reader, completeness, |reader, _| read_block(reader))
    }

    /// Suggests the block coordinates.
    pub fn suggest<'t, T>(builder: &mut SuggestionBuilder<'t, '_, T>) {
        suggest_coordinates(builder, |reader| {
            Self::read(reader, Completeness::Partial).is_ok()
        });
    }
}

impl<S> Argument<S> for BlockPosArgument {
    type Value<'a> = Coordinates;
    type Error = CoordinatesError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
        Self::read(reader, Completeness::Full)
    }
}
