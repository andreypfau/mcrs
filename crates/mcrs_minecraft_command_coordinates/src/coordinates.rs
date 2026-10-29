use crate::error::CoordinatesError;
use glam::{BVec3, DVec3, Vec3};
use mcrs_command::SEPARATOR;
use mcrs_command::{
    Argument, LibraryError, Reader, SuggestionBuilder, SuggestionText, SyntaxError,
};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_core::mth::{atan2, cos_rounded, sin_rounded, wrap_degrees};

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

/// One coordinate: `~` with an optional offset, or an absolute value that `absolute` reads.
fn read_axis(
    reader: &mut Reader<'_>,
    missing: CoordinatesError,
    expected: LibraryError,
    absolute: impl FnOnce(&mut Reader<'_>) -> Result<f64, SyntaxError<CoordinatesError>>,
) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>> {
    if reader.peek_is('^') {
        return Err(SyntaxError::argument(
            CoordinatesError::PosMixed,
            reader.cursor(),
        ));
    }
    if reader.at_end() {
        return Err(SyntaxError::argument(missing, reader.cursor()));
    }
    if reader.peek_is('~') {
        reader.skip();
        let value = if reader.at_end() || reader.peek_is(SEPARATOR) {
            0.0
        } else {
            reader.read_double()?
        };
        return Ok(WorldCoordinate {
            value,
            relative: true,
        });
    }
    if reader.peek_is(SEPARATOR) {
        return Err(SyntaxError::library(expected, reader.cursor()));
    }
    Ok(WorldCoordinate {
        value: absolute(reader)?,
        relative: false,
    })
}

/// One world coordinate (ARG-COORD-001).
fn read_world(
    reader: &mut Reader<'_>,
    center: bool,
) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>> {
    read_axis(
        reader,
        CoordinatesError::PosMissingDouble,
        LibraryError::ExpectedDouble,
        |reader| {
            let start = reader.cursor();
            let value = reader.read_double()?;
            Ok(
                if center && !reader.span_from(start).text(reader.input()).contains('.') {
                    value + 0.5
                } else {
                    value
                },
            )
        },
    )
}

fn separator(
    reader: &mut Reader<'_>,
    start: usize,
    error: CoordinatesError,
) -> Result<(), SyntaxError<CoordinatesError>> {
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
    mut read_axis: impl FnMut(&mut Reader<'_>, usize) -> Result<T, SyntaxError<CoordinatesError>>,
) -> Result<[T; 3], SyntaxError<CoordinatesError>> {
    let start = reader.cursor();
    let mut values = [unread; 3];
    for (axis, value) in values.iter_mut().enumerate() {
        if axis > 0 {
            if partial && reader.at_end() {
                break;
            }
            separator(reader, start, CoordinatesError::Pos3dIncomplete)?;
        }
        *value = read_axis(reader, axis)?;
    }
    Ok(values)
}

/// Three local values in the order left, up, forwards (ARG-COORD-002).
fn read_local(
    reader: &mut Reader<'_>,
    partial: bool,
) -> Result<[f64; 3], SyntaxError<CoordinatesError>> {
    let start = reader.cursor();
    read_axes(reader, partial, 0.0, |reader, _| {
        if reader.at_end() {
            return Err(SyntaxError::argument(
                CoordinatesError::PosMissingDouble,
                reader.cursor(),
            ));
        }
        if !reader.peek_is('^') {
            reader.set_cursor(start);
            return Err(SyntaxError::argument(CoordinatesError::PosMixed, start));
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

fn read_coordinates(
    reader: &mut Reader<'_>,
    partial: bool,
    axis: impl FnMut(&mut Reader<'_>, usize) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>>,
) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
    if reader.peek_is('^') {
        return read_local(reader, partial).map(Coordinates::Local);
    }
    let unread = WorldCoordinate {
        value: 0.0,
        relative: true,
    };
    read_axes(reader, partial, unread, axis).map(Coordinates::World)
}

/// `minecraft:vec3` (ARG-COORD-003), centred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vec3Argument;

impl Vec3Argument {
    fn read(
        &self,
        reader: &mut Reader<'_>,
        partial: bool,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
        read_coordinates(reader, partial, |reader, axis| {
            read_world(reader, axis != 1)
        })
    }

    /// ARG-COORD-005, with `~ ~ ~` as the candidate of the server. A partial read fills the
    /// fields that `r` lacks as the candidate does, so it reads the candidate in full.
    pub fn suggest<'t, T>(&self, builder: &mut SuggestionBuilder<'t, '_, T>) {
        suggest_coordinates(builder, |reader| self.read(reader, true).is_ok());
    }
}

fn suggest_coordinates<'t, T>(
    builder: &mut SuggestionBuilder<'t, '_, T>,
    reads: impl Fn(&mut Reader<'_>) -> bool,
) {
    let remaining = builder.remaining();
    let trimmed = remaining.strip_suffix(SEPARATOR).unwrap_or(remaining);
    if !trimmed.is_empty() && !reads(&mut Reader::new(trimmed)) {
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

impl<L> Argument<Source<'_, L>> for Vec3Argument {
    type Value<'a> = Coordinates;
    type Error = CoordinatesError;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &Source<'_, L>,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
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
        let yaw = read_world(reader, false)?;
        separator(reader, start, CoordinatesError::RotationIncomplete)?;
        let pitch = read_world(reader, false)?;
        Ok(RotationValue { yaw, pitch })
    }
}

/// One block coordinate (ARG-COORD-010).
fn read_block(reader: &mut Reader<'_>) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>> {
    read_axis(
        reader,
        CoordinatesError::PosMissingInt,
        LibraryError::ExpectedInt,
        |reader| Ok(f64::from(reader.read_int::<CoordinatesError>()?)),
    )
}

/// The block of a point: the floor of each axis, limited to the range of `i32`
/// (ARG-COORD-012).
pub fn block_floor(point: DVec3) -> [i32; 3] {
    [point.x, point.y, point.z].map(|axis| axis.floor() as i32)
}

/// `minecraft:block_pos` (ARG-COORD-011). The value is the coordinates; the block position
/// is [`block_floor`] of their point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockPosArgument;

impl BlockPosArgument {
    fn read(
        reader: &mut Reader<'_>,
        partial: bool,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
        read_coordinates(reader, partial, |reader, _| read_block(reader))
    }

    /// ARG-COORD-014.
    pub fn suggest<'t, T>(builder: &mut SuggestionBuilder<'t, '_, T>) {
        suggest_coordinates(builder, |reader| Self::read(reader, true).is_ok());
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
        Self::read(reader, false)
    }
}

/// Each axis of `axes` takes the block floor of its value (ARG-COORD-016).
pub fn align(axes: BVec3, point: DVec3) -> DVec3 {
    DVec3::select(axes, point.floor().as_ivec3().as_dvec3(), point)
}

/// `minecraft:swizzle` (ARG-COORD-015).
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

/// The yaw and the pitch that look from `from` to `to` (CMD-EX-018).
pub fn rotation_toward(from: DVec3, to: DVec3) -> (f32, f32) {
    let d = to - from;
    let horizontal = (d.x * d.x + d.z * d.z).sqrt();
    let pi = f64::from(core::f32::consts::PI);
    let pitch = wrap_degrees(-(atan2(d.y, horizontal) * 180.0 / pi) as f32);
    let yaw = wrap_degrees((atan2(d.z, d.x) * 180.0 / pi) as f32 - 90.0);
    (yaw, pitch)
}
