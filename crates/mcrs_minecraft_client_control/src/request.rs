use core::fmt;

use bevy_math::DVec3;
use mcrs_minecraft_client_world::movement_input::MovementInput;
use mcrs_voxel::BlockPos;

pub(crate) const MAX_TICKS: u32 = 1200;

pub(crate) const DEFAULT_RADIUS: f64 = 16.0;

pub(crate) const MAX_RADIUS: f64 = 128.0;

const LAST_SLOT: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Request<'a> {
    Ready,
    Player,
    Target,
    Entities { radius: f64 },
    Block(BlockPos),
    Look { yaw: f32, pitch: f32 },
    LookAt(DVec3),
    LookEntity(i32),
    Hold { keys: MovementInput, ticks: u32 },
    Attack { ticks: u32 },
    Slot(u8),
    Chat(&'a str),
    Command(&'a str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestError {
    Empty,
    UnknownVerb,
    MissingArgument,
    BadNumber,
    UnknownKey,
    OutOfRange,
    ExtraArgument,
}

impl fmt::Display for RequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "empty request",
            Self::UnknownVerb => {
                "unknown request; use ready, player, target, entities, block, look, look_at, look_entity, hold, attack, slot, chat or command"
            }
            Self::MissingArgument => "an argument is missing",
            Self::BadNumber => "an argument is not a number",
            Self::UnknownKey => "unknown key; use forward, back, left, right, jump, sneak or sprint",
            Self::OutOfRange => "an argument is out of range",
            Self::ExtraArgument => "too many arguments",
        })
    }
}

impl core::error::Error for RequestError {}

impl<'a> Request<'a> {
    pub fn parse(line: &'a str) -> Result<Self, RequestError> {
        let line = line.trim();
        let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim_start();
        let mut words = rest.split_whitespace();
        let request = match verb {
            "" => return Err(RequestError::Empty),
            "chat" | "command" if rest.is_empty() => return Err(RequestError::MissingArgument),
            "chat" => return Ok(Self::Chat(rest)),
            "command" => return Ok(Self::Command(rest.strip_prefix('/').unwrap_or(rest))),
            "ready" => Self::Ready,
            "player" => Self::Player,
            "target" => Self::Target,
            "entities" => Self::Entities {
                radius: optional(&mut words, DEFAULT_RADIUS)
                    .and_then(|radius| within(radius, 0.0, MAX_RADIUS))?,
            },
            "block" => Self::Block(BlockPos::new(
                number(&mut words)?,
                number(&mut words)?,
                number(&mut words)?,
            )),
            "look" => Self::Look {
                yaw: finite(&mut words)?,
                pitch: finite(&mut words).and_then(|pitch| within(pitch, -90.0, 90.0))?,
            },
            "look_at" => Self::LookAt(DVec3::new(
                finite(&mut words)?,
                finite(&mut words)?,
                finite(&mut words)?,
            )),
            "look_entity" => Self::LookEntity(number(&mut words)?),
            "hold" => Self::Hold {
                keys: keys(words.next().ok_or(RequestError::MissingArgument)?)?,
                ticks: ticks(number(&mut words)?)?,
            },
            "attack" => Self::Attack {
                ticks: optional(&mut words, 1).and_then(ticks)?,
            },
            "slot" => Self::Slot(number(&mut words).and_then(|slot| within(slot, 0, LAST_SLOT))?),
            _ => return Err(RequestError::UnknownVerb),
        };
        match words.next() {
            Some(_) => Err(RequestError::ExtraArgument),
            None => Ok(request),
        }
    }
}

fn number<'a, T: core::str::FromStr>(
    words: &mut impl Iterator<Item = &'a str>,
) -> Result<T, RequestError> {
    words
        .next()
        .ok_or(RequestError::MissingArgument)?
        .parse()
        .map_err(|_| RequestError::BadNumber)
}

fn finite<'a, T: core::str::FromStr + Into<f64> + Copy>(
    words: &mut impl Iterator<Item = &'a str>,
) -> Result<T, RequestError> {
    let value: T = number(words)?;
    if value.into().is_finite() {
        Ok(value)
    } else {
        Err(RequestError::BadNumber)
    }
}

fn optional<'a, T: core::str::FromStr>(
    words: &mut impl Iterator<Item = &'a str>,
    default: T,
) -> Result<T, RequestError> {
    match words.next() {
        Some(word) => word.parse().map_err(|_| RequestError::BadNumber),
        None => Ok(default),
    }
}

fn within<T: PartialOrd>(value: T, min: T, max: T) -> Result<T, RequestError> {
    if value >= min && value <= max {
        Ok(value)
    } else {
        Err(RequestError::OutOfRange)
    }
}

fn ticks(ticks: u32) -> Result<u32, RequestError> {
    within(ticks, 1, MAX_TICKS)
}

fn keys(list: &str) -> Result<MovementInput, RequestError> {
    let mut keys = MovementInput::EMPTY;
    for key in list.split(',') {
        let held = match key {
            "forward" => &mut keys.forward,
            "back" => &mut keys.backward,
            "left" => &mut keys.left,
            "right" => &mut keys.right,
            "jump" => &mut keys.jump,
            "sneak" => &mut keys.shift,
            "sprint" => &mut keys.sprint,
            _ => return Err(RequestError::UnknownKey),
        };
        *held = true;
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_line_reads_as_its_request_or_its_error() {
        let forward_sprint = MovementInput {
            forward: true,
            sprint: true,
            ..MovementInput::EMPTY
        };
        let cases: [(&str, Result<Request<'_>, RequestError>); 25] = [
            ("ready", Ok(Request::Ready)),
            ("  player \n", Ok(Request::Player)),
            ("entities", Ok(Request::Entities { radius: 16.0 })),
            ("entities 4.5", Ok(Request::Entities { radius: 4.5 })),
            ("entities 500", Err(RequestError::OutOfRange)),
            (
                "block 1 -64 3",
                Ok(Request::Block(BlockPos::new(1, -64, 3))),
            ),
            ("block 1 2", Err(RequestError::MissingArgument)),
            ("block 1 2 x", Err(RequestError::BadNumber)),
            ("block 1 2 3 4", Err(RequestError::ExtraArgument)),
            (
                "look 90 -30",
                Ok(Request::Look {
                    yaw: 90.0,
                    pitch: -30.0,
                }),
            ),
            ("look 0 91", Err(RequestError::OutOfRange)),
            ("look NaN 0", Err(RequestError::BadNumber)),
            ("look inf 0", Err(RequestError::BadNumber)),
            ("look_at 0 -inf 0", Err(RequestError::BadNumber)),
            (
                "look_at 0.5 64 2.5",
                Ok(Request::LookAt(DVec3::new(0.5, 64.0, 2.5))),
            ),
            ("look_entity 12", Ok(Request::LookEntity(12))),
            (
                "hold forward,sprint 20",
                Ok(Request::Hold {
                    keys: forward_sprint,
                    ticks: 20,
                }),
            ),
            ("hold fly 20", Err(RequestError::UnknownKey)),
            ("hold jump 0", Err(RequestError::OutOfRange)),
            ("attack", Ok(Request::Attack { ticks: 1 })),
            ("slot 9", Err(RequestError::OutOfRange)),
            ("chat hello  world", Ok(Request::Chat("hello  world"))),
            (
                "command /time set day",
                Ok(Request::Command("time set day")),
            ),
            ("command", Err(RequestError::MissingArgument)),
            ("jump", Err(RequestError::UnknownVerb)),
        ];
        for (line, expected) in cases {
            assert_eq!(Request::parse(line), expected, "{line:?}");
        }
        assert_eq!(Request::parse(""), Err(RequestError::Empty));
    }
}
