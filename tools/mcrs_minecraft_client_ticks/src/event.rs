use std::fmt;

use crate::ParseErrorKind;
use crate::fields::Fields;
use crate::fields::bad;
use crate::fields::parse_value;

/// One packet of a recording. A clientbound event stands at the point where the client applied
/// it: after the end of one client tick and before the packets of the next.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// `player_position`.
    Teleport(Teleport),
    /// `set_entity_motion` for the player: the decoded velocity.
    Motion([f64; 3]),
    /// `explode` with an impulse: the impulse that the client adds to its velocity.
    Explosion([f64; 3]),
    /// `player_abilities`, clientbound.
    Abilities(Abilities),
    /// One entry of `update_attributes` for the player.
    Attribute(Attribute),
    /// `update_mob_effect` for the player.
    Effect(Effect),
    /// `remove_mob_effect` for the player: the effect name.
    RemoveEffect(String),
    /// `block_update`, or one entry of `section_blocks_update`.
    Block(BlockChange),
    /// `level_chunk_with_light`: the chunk x and z.
    ChunkLoaded([i32; 2]),
    /// `forget_level_chunk`: the chunk x and z.
    ChunkForgotten([i32; 2]),
    /// `accept_teleportation`.
    AcceptTeleport(AcceptTeleport),
    /// `player_input`.
    Input(Keys),
    /// `player_command`.
    Command(Command),
    /// `player_abilities`, serverbound: the request to fly or to stop.
    Flying(bool),
    /// `use_item`.
    UseItem,
    /// `player_action` that releases the item in use.
    ReleaseUseItem,
    /// One of the four move packets.
    Move(Move),
    /// `client_tick_end`.
    TickEnd,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rotation {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Teleport {
    pub id: i32,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub rotation: Rotation,
    /// The bits of the packet that make a value relative.
    pub relative: u32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Abilities {
    pub invulnerable: bool,
    pub flying: bool,
    pub may_fly: bool,
    pub instant_build: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub base: f64,
    pub modifiers: Vec<Modifier>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    pub id: String,
    pub amount: f64,
    /// 0 adds, 1 multiplies the base, 2 multiplies the total.
    pub operation: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub name: String,
    pub amplifier: i32,
    pub duration: i32,
    pub flags: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockChange {
    pub position: [i32; 3],
    /// The state as `name[property=value,...]`.
    pub state: String,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AcceptTeleport {
    pub id: i32,
    pub position: [f64; 3],
    pub rotation: Rotation,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Keys {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub shift: bool,
    pub sprint: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    StopSleeping,
    StartSprinting,
    StopSprinting,
    StartRidingJump,
    StopRidingJump,
    OpenInventory,
    StartFallFlying,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Move {
    pub position: Option<[f64; 3]>,
    pub rotation: Option<Rotation>,
    pub on_ground: bool,
    pub horizontal_collision: bool,
}

const COMMANDS: [(Command, &str); 7] = [
    (Command::StopSleeping, "stop_sleeping"),
    (Command::StartSprinting, "start_sprinting"),
    (Command::StopSprinting, "stop_sprinting"),
    (Command::StartRidingJump, "start_riding_jump"),
    (Command::StopRidingJump, "stop_riding_jump"),
    (Command::OpenInventory, "open_inventory"),
    (Command::StartFallFlying, "start_fall_flying"),
];

struct Vec3<'a>(&'a [f64; 3]);

impl fmt::Display for Vec3<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x, y, z] = self.0;
        write!(f, "{x} {y} {z}")
    }
}

/// A flag of a type and its name in the text.
type Flag<'a> = (&'a mut bool, &'static str);

/// Writes the names whose flag is set, comma-separated, or `none`.
fn write_flags(f: &mut fmt::Formatter<'_>, flags: &[Flag<'_>]) -> fmt::Result {
    let mut first = true;
    for (_, name) in flags.iter().filter(|(set, _)| **set) {
        write!(f, "{}{name}", if first { "" } else { "," })?;
        first = false;
    }
    if first {
        f.write_str("none")?;
    }
    Ok(())
}

/// Sets each flag that a comma-separated list names; `none` is the empty list.
fn parse_flags(
    word: &str,
    field: &'static str,
    flags: &mut [Flag<'_>],
) -> Result<(), ParseErrorKind> {
    for name in word.split(',').filter(|name| *name != "none") {
        let (flag, _) = flags
            .iter_mut()
            .find(|(_, known)| *known == name)
            .ok_or_else(|| bad(field, name))?;
        **flag = true;
    }
    Ok(())
}

impl Keys {
    fn flags(&mut self) -> [Flag<'_>; 7] {
        [
            (&mut self.forward, "forward"),
            (&mut self.backward, "backward"),
            (&mut self.left, "left"),
            (&mut self.right, "right"),
            (&mut self.jump, "jump"),
            (&mut self.shift, "shift"),
            (&mut self.sprint, "sprint"),
        ]
    }
}

impl Abilities {
    fn flags(&mut self) -> [Flag<'_>; 4] {
        [
            (&mut self.invulnerable, "invulnerable"),
            (&mut self.flying, "flying"),
            (&mut self.may_fly, "may_fly"),
            (&mut self.instant_build, "instant_build"),
        ]
    }
}

impl Move {
    fn flags(&mut self) -> [Flag<'_>; 2] {
        [
            (&mut self.on_ground, "on_ground"),
            (&mut self.horizontal_collision, "horizontal_collision"),
        ]
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Teleport(teleport) => write!(
                f,
                "< teleport {} {} {} {} {} {}",
                teleport.id,
                Vec3(&teleport.position),
                Vec3(&teleport.velocity),
                teleport.rotation.yaw,
                teleport.rotation.pitch,
                teleport.relative
            ),
            Self::Motion(velocity) => write!(f, "< motion {}", Vec3(velocity)),
            Self::Explosion(impulse) => write!(f, "< explosion {}", Vec3(impulse)),
            Self::Abilities(abilities) => {
                f.write_str("< abilities ")?;
                write_flags(f, &{ *abilities }.flags())?;
                write!(f, " {} {}", abilities.flying_speed, abilities.walking_speed)
            }
            Self::Attribute(attribute) => {
                write!(f, "< attribute {} {}", attribute.name, attribute.base)?;
                for modifier in &attribute.modifiers {
                    write!(
                        f,
                        " {} {} {}",
                        modifier.id, modifier.amount, modifier.operation
                    )?;
                }
                Ok(())
            }
            Self::Effect(effect) => write!(
                f,
                "< effect {} {} {} {}",
                effect.name, effect.amplifier, effect.duration, effect.flags
            ),
            Self::RemoveEffect(name) => write!(f, "< remove_effect {name}"),
            Self::Block(change) => {
                let [x, y, z] = change.position;
                write!(f, "< block {x} {y} {z} {}", change.state)
            }
            Self::ChunkLoaded([x, z]) => write!(f, "< chunk {x} {z}"),
            Self::ChunkForgotten([x, z]) => write!(f, "< forget {x} {z}"),
            Self::AcceptTeleport(accept) => write!(
                f,
                "> accept_teleport {} {} {} {}",
                accept.id,
                Vec3(&accept.position),
                accept.rotation.yaw,
                accept.rotation.pitch
            ),
            Self::Input(keys) => {
                f.write_str("> input ")?;
                write_flags(f, &{ *keys }.flags())
            }
            Self::Command(command) => {
                let name = COMMANDS
                    .iter()
                    .find(|(known, _)| known == command)
                    .map_or("", |(_, name)| name);
                write!(f, "> command {name}")
            }
            Self::Flying(flying) => write!(f, "> flying {flying}"),
            Self::UseItem => f.write_str("> use_item"),
            Self::ReleaseUseItem => f.write_str("> release_use_item"),
            Self::Move(step) => {
                f.write_str("> move ")?;
                match step.position {
                    Some([x, y, z]) => write!(f, "{x},{y},{z} ")?,
                    None => f.write_str("- ")?,
                }
                match step.rotation {
                    Some(rotation) => write!(f, "{},{} ", rotation.yaw, rotation.pitch)?,
                    None => f.write_str("- ")?,
                }
                write_flags(f, &{ *step }.flags())
            }
            Self::TickEnd => f.write_str("> tick"),
        }
    }
}

impl Event {
    /// Parses one event line, the direction mark included.
    pub(crate) fn parse(line: &str) -> Result<Self, ParseErrorKind> {
        let mut fields = Fields::new(line);
        let direction = fields.word("direction")?;
        let kind = fields.word("kind")?;
        let event = match (direction, kind) {
            ("<", "teleport") => Self::Teleport(Teleport {
                id: fields.value("id")?,
                position: fields.array3("position")?,
                velocity: fields.array3("velocity")?,
                rotation: Rotation {
                    yaw: fields.value("yaw")?,
                    pitch: fields.value("pitch")?,
                },
                relative: fields.value("relative")?,
            }),
            ("<", "motion") => Self::Motion(fields.array3("velocity")?),
            ("<", "explosion") => Self::Explosion(fields.array3("impulse")?),
            ("<", "abilities") => parse_abilities(&mut fields)?,
            ("<", "attribute") => {
                let name = fields.word("name")?.to_owned();
                let base = fields.value("base")?;
                let mut modifiers = Vec::new();
                while let Some(id) = fields.next_word() {
                    modifiers.push(Modifier {
                        id: id.to_owned(),
                        amount: fields.value("amount")?,
                        operation: fields.value("operation")?,
                    });
                }
                Self::Attribute(Attribute {
                    name,
                    base,
                    modifiers,
                })
            }
            ("<", "effect") => Self::Effect(Effect {
                name: fields.word("name")?.to_owned(),
                amplifier: fields.value("amplifier")?,
                duration: fields.value("duration")?,
                flags: fields.value("flags")?,
            }),
            ("<", "remove_effect") => Self::RemoveEffect(fields.word("name")?.to_owned()),
            ("<", "block") => Self::Block(BlockChange {
                position: fields.array3("position")?,
                state: fields.word("state")?.to_owned(),
            }),
            ("<", "chunk") => Self::ChunkLoaded([fields.value("x")?, fields.value("z")?]),
            ("<", "forget") => Self::ChunkForgotten([fields.value("x")?, fields.value("z")?]),
            (">", "accept_teleport") => Self::AcceptTeleport(AcceptTeleport {
                id: fields.value("id")?,
                position: fields.array3("position")?,
                rotation: Rotation {
                    yaw: fields.value("yaw")?,
                    pitch: fields.value("pitch")?,
                },
            }),
            (">", "input") => parse_input(&mut fields)?,
            (">", "command") => {
                let name = fields.word("command")?;
                let Some((command, _)) = COMMANDS.iter().find(|(_, known)| *known == name) else {
                    return Err(bad("command", name));
                };
                Self::Command(*command)
            }
            (">", "flying") => Self::Flying(fields.value("flying")?),
            (">", "use_item") => Self::UseItem,
            (">", "release_use_item") => Self::ReleaseUseItem,
            (">", "move") => parse_move(&mut fields)?,
            (">", "tick") => Self::TickEnd,
            _ => return Err(ParseErrorKind::UnknownEvent(format!("{direction} {kind}"))),
        };
        fields.end()?;
        Ok(event)
    }
}

fn parse_abilities(fields: &mut Fields<'_>) -> Result<Event, ParseErrorKind> {
    let mut abilities = Abilities {
        invulnerable: false,
        flying: false,
        may_fly: false,
        instant_build: false,
        flying_speed: 0.0,
        walking_speed: 0.0,
    };
    parse_flags(fields.word("flags")?, "flags", &mut abilities.flags())?;
    abilities.flying_speed = fields.value("flying_speed")?;
    abilities.walking_speed = fields.value("walking_speed")?;
    Ok(Event::Abilities(abilities))
}

fn parse_input(fields: &mut Fields<'_>) -> Result<Event, ParseErrorKind> {
    let mut keys = Keys::default();
    parse_flags(fields.word("keys")?, "keys", &mut keys.flags())?;
    Ok(Event::Input(keys))
}

fn parse_move(fields: &mut Fields<'_>) -> Result<Event, ParseErrorKind> {
    let position = match fields.word("position")? {
        "-" => None,
        word => {
            let mut parts = word.split(',');
            let mut axis = || -> Result<f64, ParseErrorKind> {
                parse_value("position", parts.next().unwrap_or_default())
            };
            Some([axis()?, axis()?, axis()?])
        }
    };
    let rotation = match fields.word("rotation")? {
        "-" => None,
        word => {
            let (yaw, pitch) = word.split_once(',').ok_or_else(|| bad("rotation", word))?;
            Some(Rotation {
                yaw: parse_value("rotation", yaw)?,
                pitch: parse_value("rotation", pitch)?,
            })
        }
    };
    let mut step = Move {
        position,
        rotation,
        on_ground: false,
        horizontal_collision: false,
    };
    parse_flags(fields.word("flags")?, "flags", &mut step.flags())?;
    Ok(Event::Move(step))
}
