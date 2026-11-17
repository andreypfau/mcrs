use alloc::vec::Vec;

use glam::Vec3;
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, Packet, VarU64};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovePlayer {
    pub runtime_id: VarU64,
    pub position: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub head_yaw: f32,
    pub mode: MoveMode,
    pub on_ground: bool,
    pub ridden_runtime_id: VarU64,
    pub tick: VarU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MoveMode {
    Normal,
    Reset,
    Teleport {
        cause: TeleportCause,
        source_entity_type: i32,
    },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = i32)]
#[non_exhaustive]
pub enum TeleportCause {
    Unknown = 0,
    Projectile = 1,
    ChorusFruit = 2,
    Command = 3,
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
