use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::build::{Tags, Validate};
use crate::error::{SnbtError, SnbtErrorKind};
use crate::reader::{Reader, Shape};

pub fn read_tag(input: &str, start: usize) -> Result<(NbtTag, usize), SnbtError> {
    let mut reader = Reader::new(input, start, Tags);
    let (tag, _) = reader.read()?;
    Ok((tag, reader.position()))
}

pub fn read_compound(input: &str, start: usize) -> Result<(NbtCompound, usize), SnbtError> {
    let mut reader = Reader::new(input, start, Tags);
    match reader.read()? {
        (NbtTag::Compound(compound), _) => Ok((compound, reader.position())),
        _ => Err(expected_compound(reader.position())),
    }
}

pub fn check_compound(input: &str, start: usize) -> Result<usize, SnbtError> {
    let mut reader = Reader::new(input, start, Validate);
    match reader.read()? {
        ((), Shape::Compound) => Ok(reader.position()),
        _ => Err(expected_compound(reader.position())),
    }
}

fn expected_compound(position: usize) -> SnbtError {
    SnbtError {
        kind: SnbtErrorKind::ExpectedCompound,
        position,
    }
}

fn full_text(input: &str) -> Result<(NbtTag, usize), SnbtError> {
    let mut reader = Reader::new(input, 0, Tags);
    let (tag, _) = reader.read()?;
    let end = reader.position();
    reader.skip_whitespace();
    if !reader.rest().is_empty() {
        return Err(SnbtError {
            kind: SnbtErrorKind::Trailing,
            position: reader.position(),
        });
    }
    Ok((tag, end))
}

pub fn parse_tag(input: &str) -> Result<NbtTag, SnbtError> {
    full_text(input).map(|(tag, _)| tag)
}

pub fn parse_compound(input: &str) -> Result<NbtCompound, SnbtError> {
    match full_text(input)? {
        (NbtTag::Compound(compound), _) => Ok(compound),
        (_, end) => Err(expected_compound(end)),
    }
}
