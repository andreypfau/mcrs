use crate::error::CoordinatesError;
use glam::{BVec3, DVec3, Vec3};
use mcrs_command::SEPARATOR;
use mcrs_command::{
    Argument, LibraryError, Reader, SuggestionBuilder, SuggestionText, SyntaxError,
};
use mcrs_minecraft_command::Source;
use mcrs_minecraft_core::mth::{atan2, cos_rounded, sin_rounded, wrap_degrees};
use mcrs_minecraft_protocol::Look;

/// One axis of a world position or of a rotation: a value, or an offset from the source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldCoordinate {
    /// The absolute value, or the offset when `relative` is set.
    pub value: f64,
    /// Whether `value` is an offset from the value of the source (`~`).
    pub relative: bool,
}

impl WorldCoordinate {
    /// The value of the axis against `origin`, the value of the source on that axis.
    #[must_use]
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

/// Where an absolute integer value of a world coordinate lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Centre {
    /// An integer value moves to the centre of its block.
    Block,
    /// The value stays as written.
    Exact,
}

/// Whether a read of three axes accepts the end of the input after a complete axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Completeness {
    Full,
    Partial,
}

/// One world coordinate (ARG-COORD-001).
fn read_world(
    reader: &mut Reader<'_>,
    centre: Centre,
) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>> {
    read_axis(
        reader,
        CoordinatesError::PosMissingDouble,
        LibraryError::ExpectedDouble,
        |reader| {
            let start = reader.cursor();
            let value = reader.read_double()?;
            Ok(
                if centre == Centre::Block
                    && !reader.span_from(start).text(reader.input()).contains('.')
                {
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
    completeness: Completeness,
    unread: T,
    mut read_axis: impl FnMut(&mut Reader<'_>, usize) -> Result<T, SyntaxError<CoordinatesError>>,
) -> Result<[T; 3], SyntaxError<CoordinatesError>> {
    let start = reader.cursor();
    let mut values = [unread; 3];
    for (axis, value) in values.iter_mut().enumerate() {
        if axis > 0 {
            if completeness == Completeness::Partial && reader.at_end() {
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
    completeness: Completeness,
) -> Result<[f64; 3], SyntaxError<CoordinatesError>> {
    let start = reader.cursor();
    read_axes(reader, completeness, 0.0, |reader, _| {
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

/// A position as a command writes it, resolved against a source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinates {
    /// The x, y and z axes, each absolute or relative to the position of the source.
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
    let (cos_yaw, sin_yaw) = (
        cos_rounded(radians(yaw + 90.0)),
        sin_rounded(radians(yaw + 90.0)),
    );
    let (cos_pitch, sin_pitch) = (cos_rounded(radians(-pitch)), sin_rounded(radians(-pitch)));
    let (cos_up_pitch, sin_up_pitch) = (
        cos_rounded(radians(-pitch + 90.0)),
        sin_rounded(radians(-pitch + 90.0)),
    );
    let forward = Vec3::new(cos_yaw * cos_pitch, sin_pitch, sin_yaw * cos_pitch).as_dvec3();
    let upward = Vec3::new(cos_yaw * cos_up_pitch, sin_up_pitch, sin_yaw * cos_up_pitch).as_dvec3();
    let leftward = -forward.cross(upward);
    anchor + forward * forwards + upward * up + leftward * left
}

impl Coordinates {
    /// The point the coordinates name against `source`.
    #[must_use]
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

    /// The axes that are relative to the source (ARG-COORD-009).
    #[must_use]
    pub fn relative(&self) -> BVec3 {
        match self {
            Coordinates::World(axes) => BVec3::from(axes.map(|axis| axis.relative)),
            Coordinates::Local(_) => BVec3::TRUE,
        }
    }
}

fn read_coordinates(
    reader: &mut Reader<'_>,
    completeness: Completeness,
    axis: impl FnMut(&mut Reader<'_>, usize) -> Result<WorldCoordinate, SyntaxError<CoordinatesError>>,
) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
    if reader.peek_is('^') {
        return read_local(reader, completeness).map(Coordinates::Local);
    }
    let unread = WorldCoordinate {
        value: 0.0,
        relative: true,
    };
    read_axes(reader, completeness, unread, axis).map(Coordinates::World)
}

/// `minecraft:vec3` (ARG-COORD-003), centred.
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

    /// Suggests the coordinates (ARG-COORD-005), with `~ ~ ~` as the candidate of the server.
    /// A partial read accepts the axes the input lacks, as the candidate fills them.
    pub fn suggest<'t, T>(&self, builder: &mut SuggestionBuilder<'t, '_, T>) {
        suggest_coordinates(builder, |reader| {
            self.read(reader, Completeness::Partial).is_ok()
        });
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
        self.read(reader, Completeness::Full)
    }
}

/// A rotation as a command writes it, resolved against a source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationValue {
    /// The yaw in degrees.
    pub yaw: WorldCoordinate,
    /// The pitch in degrees.
    pub pitch: WorldCoordinate,
}

impl RotationValue {
    /// The yaw and the pitch against the source (ARG-COORD-008).
    #[must_use]
    pub fn resolve<L>(&self, source: &Source<'_, L>) -> Look {
        Look {
            yaw: self.yaw.resolve(f64::from(source.yaw)) as f32,
            pitch: self.pitch.resolve(f64::from(source.pitch)) as f32,
        }
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
        let yaw = read_world(reader, Centre::Exact)?;
        separator(reader, start, CoordinatesError::RotationIncomplete)?;
        let pitch = read_world(reader, Centre::Exact)?;
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
#[must_use]
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
        completeness: Completeness,
    ) -> Result<Coordinates, SyntaxError<CoordinatesError>> {
        read_coordinates(reader, completeness, |reader, _| read_block(reader))
    }

    /// Suggests the block coordinates (ARG-COORD-014).
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

/// Each axis of `axes` takes the block floor of its value (ARG-COORD-016).
#[must_use]
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
#[must_use]
pub fn rotation_toward(from: DVec3, to: DVec3) -> Look {
    let delta = to - from;
    let horizontal = (delta.x * delta.x + delta.z * delta.z).sqrt();
    let pi = f64::from(core::f32::consts::PI);
    Look {
        yaw: wrap_degrees((atan2(delta.z, delta.x) * 180.0 / pi) as f32 - 90.0),
        pitch: wrap_degrees(-(atan2(delta.y, horizontal) * 180.0 / pi) as f32),
    }
}
