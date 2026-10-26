use crate::error::{ArgumentError, CommandSyntaxError};
use crate::source::Source;
use glam::{DVec3, Vec3};
use mcrs_command::reader::SEPARATOR;
use mcrs_command::{
    Argument, LibraryError, Reader, SuggestionBuilder, SuggestionText, SyntaxError,
};
use mcrs_minecraft_core::mth::{cos_rounded, sin_rounded};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCoordinate {
    pub value: f64,
    pub relative: bool,
}

impl WorldCoordinate {
    pub fn resolve(self, origin: f64) -> f64 {
        if self.relative {
            self.value + origin
        } else {
            self.value
        }
    }
}

fn missing(reader: &Reader<'_>) -> CommandSyntaxError {
    SyntaxError::argument(ArgumentError::PosMissingDouble, reader.cursor())
}

/// One world coordinate (ARG-COORD-001).
fn read_world(
    reader: &mut Reader<'_>,
    center: bool,
) -> Result<WorldCoordinate, CommandSyntaxError> {
    if reader.peek_is('^') {
        return Err(SyntaxError::argument(
            ArgumentError::PosMixed,
            reader.cursor(),
        ));
    }
    if reader.at_end() {
        return Err(missing(reader));
    }
    let relative = reader.peek_is('~');
    if relative {
        reader.skip();
        if reader.at_end() || reader.peek_is(SEPARATOR) {
            return Ok(WorldCoordinate {
                value: 0.0,
                relative,
            });
        }
    } else if reader.peek_is(SEPARATOR) {
        return Err(SyntaxError::library(
            LibraryError::ExpectedDouble,
            reader.cursor(),
        ));
    }
    let start = reader.cursor();
    let mut value = reader.read_double()?;
    if !relative && center && !reader.span_from(start).text(reader.input()).contains('.') {
        value += 0.5;
    }
    Ok(WorldCoordinate { value, relative })
}

fn separator(
    reader: &mut Reader<'_>,
    start: usize,
    error: ArgumentError,
) -> Result<(), CommandSyntaxError> {
    if reader.peek_is(SEPARATOR) {
        reader.skip();
        Ok(())
    } else {
        reader.set_cursor(start);
        Err(SyntaxError::argument(error, start))
    }
}

/// Three axes apart by one separator each. A partial read stops without an error at the end
/// of the input after a complete axis; the axes it did not read keep `unread`.
fn read_axes<T: Copy>(
    reader: &mut Reader<'_>,
    partial: bool,
    unread: T,
    mut read_axis: impl FnMut(&mut Reader<'_>, usize) -> Result<T, CommandSyntaxError>,
) -> Result<[T; 3], CommandSyntaxError> {
    let start = reader.cursor();
    let mut values = [unread; 3];
    for (axis, value) in values.iter_mut().enumerate() {
        if axis > 0 {
            if partial && reader.at_end() {
                break;
            }
            separator(reader, start, ArgumentError::Pos3dIncomplete)?;
        }
        *value = read_axis(reader, axis)?;
    }
    Ok(values)
}

/// Three local values in the order left, up, forwards (ARG-COORD-002).
fn read_local(reader: &mut Reader<'_>, partial: bool) -> Result<[f64; 3], CommandSyntaxError> {
    let start = reader.cursor();
    read_axes(reader, partial, 0.0, |reader, _| {
        if reader.at_end() {
            return Err(missing(reader));
        }
        if !reader.peek_is('^') {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(ArgumentError::PosMixed, start));
        }
        reader.skip();
        if reader.at_end() || reader.peek_is(SEPARATOR) {
            Ok(0.0)
        } else {
            reader.read_double()
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinates {
    World([WorldCoordinate; 3]),
    /// Left, up and forwards along the axes of the source rotation.
    Local([f64; 3]),
}

fn radians(degrees: f32) -> f64 {
    f64::from(degrees * (core::f64::consts::PI / 180.0) as f32)
}

/// The point at local offsets from `anchor`, as ARG-COORD-007 computes it.
pub(crate) fn local_position(
    anchor: DVec3,
    yaw: f32,
    pitch: f32,
    [left, up, forwards]: [f64; 3],
) -> DVec3 {
    let (cy, sy) = (
        cos_rounded(radians(yaw + 90.0)),
        sin_rounded(radians(yaw + 90.0)),
    );
    let (cx, sx) = (cos_rounded(radians(-pitch)), sin_rounded(radians(-pitch)));
    let (cu, su) = (
        cos_rounded(radians(-pitch + 90.0)),
        sin_rounded(radians(-pitch + 90.0)),
    );
    let f = Vec3::new(cy * cx, sx, sy * cx).as_dvec3();
    let u = Vec3::new(cy * cu, su, sy * cu).as_dvec3();
    let l = -f.cross(u);
    anchor + f * forwards + u * up + l * left
}

impl Coordinates {
    pub fn position<L>(&self, source: &Source<'_, L>) -> DVec3 {
        match self {
            Coordinates::World([x, y, z]) => DVec3::new(
                x.resolve(source.position.x),
                y.resolve(source.position.y),
                z.resolve(source.position.z),
            ),
            Coordinates::Local(local) => {
                local_position(source.anchor_position(), source.yaw, source.pitch, *local)
            }
        }
    }

    /// ARG-COORD-009.
    pub fn relative(&self) -> [bool; 3] {
        match self {
            Coordinates::World(axes) => axes.map(|axis| axis.relative),
            Coordinates::Local(_) => [true; 3],
        }
    }
}

/// `minecraft:vec3` (ARG-COORD-003), centred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vec3Argument;

impl Vec3Argument {
    fn read(
        &self,
        reader: &mut Reader<'_>,
        partial: bool,
    ) -> Result<Coordinates, CommandSyntaxError> {
        if reader.peek_is('^') {
            return read_local(reader, partial).map(Coordinates::Local);
        }
        let unread = WorldCoordinate {
            value: 0.0,
            relative: true,
        };
        read_axes(reader, partial, unread, |reader, axis| {
            read_world(reader, axis != 1)
        })
        .map(Coordinates::World)
    }

    /// ARG-COORD-005, with `~ ~ ~` as the candidate of the server. A partial read fills the
    /// fields that `r` lacks as the candidate does, so it reads the candidate in full.
    pub fn suggest<'t, T>(&self, builder: &mut SuggestionBuilder<'t, '_, T>) {
        let remaining = builder.remaining();
        let trimmed = remaining.strip_suffix(SEPARATOR).unwrap_or(remaining);
        if !trimmed.is_empty() && self.read(&mut Reader::new(trimmed), true).is_err() {
            return;
        }
        let [one, two, three] = if remaining.starts_with('^') {
            ["^", " ^", " ^ ^"]
        } else {
            ["~", " ~", " ~ ~"]
        };
        match trimmed.split(SEPARATOR).count() {
            _ if trimmed.is_empty() => {
                builder.suggest(SuggestionText::new(one), None);
                builder.suggest(SuggestionText::concat(one, two), None);
                builder.suggest(SuggestionText::concat(one, three), None);
            }
            1 => {
                builder.suggest(SuggestionText::concat(trimmed, two), None);
                builder.suggest(SuggestionText::concat(trimmed, three), None);
            }
            2 => builder.suggest(SuggestionText::concat(trimmed, two), None),
            _ => {}
        }
    }
}

impl<L> Argument<Source<'_, L>> for Vec3Argument {
    type Value<'a> = Coordinates;
    type Error = ArgumentError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &Source<'_, L>,
    ) -> Result<Coordinates, CommandSyntaxError> {
        self.read(reader, false)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationValue {
    pub yaw: WorldCoordinate,
    pub pitch: WorldCoordinate,
}

impl RotationValue {
    /// The yaw and the pitch against the source (ARG-COORD-008).
    pub fn resolve<L>(&self, source: &Source<'_, L>) -> (f32, f32) {
        (
            self.yaw.resolve(f64::from(source.yaw)) as f32,
            self.pitch.resolve(f64::from(source.pitch)) as f32,
        )
    }
}

/// `minecraft:rotation` (ARG-COORD-004).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RotationArgument;

impl<S> Argument<S> for RotationArgument {
    type Value<'a> = RotationValue;
    type Error = ArgumentError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<RotationValue, CommandSyntaxError> {
        let start = reader.cursor();
        if reader.at_end() {
            return Err(SyntaxError::argument(
                ArgumentError::RotationIncomplete,
                reader.cursor(),
            ));
        }
        let yaw = read_world(reader, false)?;
        separator(reader, start, ArgumentError::RotationIncomplete)?;
        let pitch = read_world(reader, false)?;
        Ok(RotationValue { yaw, pitch })
    }
}
