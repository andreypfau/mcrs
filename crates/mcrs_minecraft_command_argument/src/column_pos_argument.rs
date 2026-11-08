use crate::coordinate_reader::{read_block, separator};
use crate::coordinates_error::CoordinatesError;
use crate::world_coordinate::WorldCoordinate;
use mcrs_command::{Argument, Reader, SyntaxError};
use mcrs_minecraft_command::Source;

/// `minecraft:column_pos`: two block coordinates, x and z.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColumnPosArgument;

/// The value of a `column_pos`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnCoordinates {
    pub x: WorldCoordinate,
    pub z: WorldCoordinate,
}

impl ColumnCoordinates {
    /// The block x and z against the position of `source`.
    #[must_use]
    pub fn block<L>(&self, source: &Source<'_, L>) -> [i32; 2] {
        [
            self.x.resolve(source.position.x).floor() as i32,
            self.z.resolve(source.position.z).floor() as i32,
        ]
    }
}

impl<S> Argument<S> for ColumnPosArgument {
    type Value<'a> = ColumnCoordinates;
    type Error = CoordinatesError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<ColumnCoordinates, SyntaxError<CoordinatesError>> {
        let start = reader.cursor();
        let x = read_block(reader)?;
        separator(reader, start, CoordinatesError::Pos2dIncomplete)?;
        let z = read_block(reader)?;
        Ok(ColumnCoordinates { x, z })
    }
}
