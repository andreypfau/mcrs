use crate::coordinate_reader::{
    Centre, Completeness, read_coordinates, read_world, suggest_coordinates,
};
use crate::coordinates::Coordinates;
use crate::coordinates_error::CoordinatesError;
use mcrs_command::{Argument, Reader, SuggestionBuilder, SyntaxError};
use mcrs_minecraft_command::Source;

/// `minecraft:vec3`, centred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vec3Argument;

impl Vec3Argument {
    fn read(
        &self,
        reader: &mut Reader<'_>,
        completeness: Completeness,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
        read_coordinates(reader, completeness, |reader, axis| {
            let centre = if axis == 1 {
                Centre::Exact
            } else {
                Centre::Block
            };
            read_world(reader, centre)
        })
    }

    /// Suggests the coordinates, with `~ ~ ~` as the candidate of the server.
    /// A partial read accepts the axes the input lacks, as the candidate fills them.
    pub fn suggest<'t, T>(&self, builder: &mut SuggestionBuilder<'t, '_, T>) {
        suggest_coordinates(builder, |reader| {
            self.read(reader, Completeness::Partial).is_ok()
        });
    }
}

impl<L> Argument<Source<'_, L>> for Vec3Argument {
    type Value<'a> = Coordinates;
    type Error = CoordinatesError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &Source<'_, L>,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
        self.read(reader, Completeness::Full)
    }
}
