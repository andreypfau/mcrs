use alloc::vec::Vec;

use glam::Vec3;
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, Packet, VarU32, VarU64};

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

impl Packet for PlayerAuthInput {
    const ID: u32 = 0x90;
}

impl BedrockEncode for PlayerAuthInput {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
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
