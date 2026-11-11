use alloc::vec::Vec;

use glam::Vec3;
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, Packet, VarU64};

/// Moves a player: the player of the client, or another player that the client shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovePlayer {
    /// The runtime id of the player that moves.
    pub runtime_id: VarU64,
    /// The eye position.
    pub position: Vec3,
    /// The pitch in degrees.
    pub pitch: f32,
    /// The yaw in degrees.
    pub yaw: f32,
    /// The head yaw in degrees.
    pub head_yaw: f32,
    /// The mode, with the teleport fields when the mode is a teleport.
    pub mode: MoveMode,
    /// Whether the player stands on the ground.
    pub on_ground: bool,
    /// The runtime id of the entity that the player rides; 0 when it rides nothing.
    pub ridden_runtime_id: VarU64,
    /// For the player of the client, the client tick of the last `PlayerAuthInput` that the
    /// server processed; otherwise 0.
    pub tick: VarU64,
}

/// The mode of [`MovePlayer`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MoveMode {
    /// The movement of another player.
    Normal,
    /// Moves the player of the client to the position.
    Reset,
    /// Teleports the player of the client to the position.
    Teleport {
        /// The cause of the teleport.
        cause: TeleportCause,
        /// The entity type id of the source; 0 for none.
        source_entity_type: i32,
    },
    /// Turns the head only.
    HeadRotation,
}

impl MoveMode {
    const fn wire_id(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Reset => 1,
            Self::Teleport { .. } => 2,
            Self::HeadRotation => 3,
        }
    }
}

/// The cause of a [`MoveMode::Teleport`], as an `i32le`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = i32)]
#[non_exhaustive]
pub enum TeleportCause {
    /// The cause is not known.
    Unknown = 0,
    /// A projectile.
    Projectile = 1,
    /// A chorus fruit.
    ChorusFruit = 2,
    /// A command.
    Command = 3,
    /// A behaviour.
    Behaviour = 4,
}

impl Packet for MovePlayer {
    const ID: u16 = 0x13;
}

impl BedrockEncode for MovePlayer {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        self.runtime_id.encode(w)?;
        self.position.encode(w)?;
        self.pitch.encode(w)?;
        self.yaw.encode(w)?;
        self.head_yaw.encode(w)?;
        self.mode.wire_id().encode(w)?;
        self.on_ground.encode(w)?;
        self.ridden_runtime_id.encode(w)?;
        if let MoveMode::Teleport {
            cause,
            source_entity_type,
        } = self.mode
        {
            cause.encode(w)?;
            source_entity_type.encode(w)?;
        }
        self.tick.encode(w)
    }
}

impl<'a> BedrockDecode<'a> for MovePlayer {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let runtime_id = VarU64::decode(r)?;
        let position = Vec3::decode(r)?;
        let pitch = f32::decode(r)?;
        let yaw = f32::decode(r)?;
        let head_yaw = f32::decode(r)?;
        let mode_id = u8::decode(r)?;
        let on_ground = bool::decode(r)?;
        let ridden_runtime_id = VarU64::decode(r)?;
        let mode = match mode_id {
            0 => MoveMode::Normal,
            1 => MoveMode::Reset,
            2 => MoveMode::Teleport {
                cause: TeleportCause::decode(r)?,
                source_entity_type: i32::decode(r)?,
            },
            3 => MoveMode::HeadRotation,
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        };
        Ok(Self {
            runtime_id,
            position,
            pitch,
            yaw,
            head_yaw,
            mode,
            on_ground,
            ridden_runtime_id,
            tick: VarU64::decode(r)?,
        })
    }
}
