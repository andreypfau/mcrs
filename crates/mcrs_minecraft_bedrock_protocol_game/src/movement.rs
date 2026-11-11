//! Bedrock Edition movement and input packets.

use crate::packet_header::{read_packet_header, write_packet_header};
use glam::Vec3;
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarU32, VarU64};

use crate::packet_ids;

/// Movement mode in `MovePlayer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MoveMode {
    Normal,
    Reset,
    Teleport { cause: i32, source_entity_type: i32 },
    Rotation,
}

impl MoveMode {
    const fn id(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Reset => 1,
            Self::Teleport { .. } => 2,
            Self::Rotation => 3,
        }
    }
}

/// Serverbound or clientbound player movement packet.
#[derive(Clone, Debug, PartialEq)]
pub struct MovePlayer {
    pub runtime_entity_id: VarU64,
    pub position: Vec3,
    pub pitch: f32,
    pub yaw: f32,
    pub head_yaw: f32,
    pub mode: MoveMode,
    pub on_ground: bool,
    pub riding_runtime_entity_id: VarU64,
    pub tick: u64,
}

impl BedrockEncode for MovePlayer {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::MOVE_PLAYER, w)?;
        self.runtime_entity_id.encode(w)?;
        self.position.x.encode(w)?;
        self.position.y.encode(w)?;
        self.position.z.encode(w)?;
        self.pitch.encode(w)?;
        self.yaw.encode(w)?;
        self.head_yaw.encode(w)?;
        self.mode.id().encode(w)?;
        self.on_ground.encode(w)?;
        self.riding_runtime_entity_id.encode(w)?;
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
        read_packet_header(r, packet_ids::MOVE_PLAYER)?;
        let runtime_entity_id = VarU64::decode(r)?;
        let position = Vec3::new(f32::decode(r)?, f32::decode(r)?, f32::decode(r)?);
        let pitch = f32::decode(r)?;
        let yaw = f32::decode(r)?;
        let head_yaw = f32::decode(r)?;
        let mode_id = u8::decode(r)?;
        let on_ground = bool::decode(r)?;
        let riding_runtime_entity_id = VarU64::decode(r)?;
        let mode = match mode_id {
            0 => MoveMode::Normal,
            1 => MoveMode::Reset,
            2 => MoveMode::Teleport {
                cause: i32::decode(r)?,
                source_entity_type: i32::decode(r)?,
            },
            3 => MoveMode::Rotation,
            other => return Err(Error::UnknownEnumValue(i64::from(other))),
        };
        Ok(Self {
            runtime_entity_id,
            position,
            pitch,
            yaw,
            head_yaw,
            mode,
            on_ground,
            riding_runtime_entity_id,
            tick: u64::decode(r)?,
        })
    }
}

/// Client authoritative input packet carrying movement and camera inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerAuthInput {
    pub pitch: f32,
    pub yaw: f32,
    pub position: Vec3,
    pub move_vec_x: f32,
    pub move_vec_z: f32,
    pub head_yaw: f32,
    pub input_data: u64,
    pub input_mode: VarU32,
    pub play_mode: VarU32,
    pub tick: VarU64,
}

impl BedrockEncode for PlayerAuthInput {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::PLAYER_AUTH_INPUT, w)?;
        self.pitch.encode(w)?;
        self.yaw.encode(w)?;
        self.position.x.encode(w)?;
        self.position.y.encode(w)?;
        self.position.z.encode(w)?;
        self.move_vec_x.encode(w)?;
        self.move_vec_z.encode(w)?;
        self.head_yaw.encode(w)?;
        self.input_data.encode(w)?;
        self.input_mode.encode(w)?;
        self.play_mode.encode(w)?;
        self.tick.encode(w)?;
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for PlayerAuthInput {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::PLAYER_AUTH_INPUT)?;
        let pitch = f32::decode(r)?;
        let yaw = f32::decode(r)?;
        let x = f32::decode(r)?;
        let y = f32::decode(r)?;
        let z = f32::decode(r)?;
        let move_vec_x = f32::decode(r)?;
        let move_vec_z = f32::decode(r)?;
        let head_yaw = f32::decode(r)?;
        let input_data = u64::decode(r)?;
        let input_mode = VarU32::decode(r)?;
        let play_mode = VarU32::decode(r)?;
        let tick = VarU64::decode(r)?;

        Ok(Self {
            pitch,
            yaw,
            position: Vec3::new(x, y, z),
            move_vec_x,
            move_vec_z,
            head_yaw,
            input_data,
            input_mode,
            play_mode,
            tick,
        })
    }
}
