//! StartGame packet initialization for Bedrock Edition clients.

use crate::packet_header::{read_packet_header, write_packet_header};
use glam::Vec3;
use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, VarI32, VarI64, VarU32, VarU64,
};

use crate::packet_ids;

/// Server packet initializing world state, player identity, and generation settings.
#[derive(Clone, Debug, PartialEq)]
pub struct StartGame {
    /// Unique entity ID of the player.
    pub entity_id: VarI64,
    /// Runtime entity ID of the player.
    pub runtime_entity_id: VarU64,
    /// Player gamemode (0: Survival, 1: Creative, 2: Adventure, 3: Spectator).
    pub player_gamemode: VarI32,
    /// Player initial spawn position.
    pub player_position: Vec3,
    /// Initial pitch rotation.
    pub pitch: f32,
    /// Initial yaw rotation.
    pub yaw: f32,
    /// World random seed.
    pub seed: i64,
    /// Dimension ID (0: Overworld, 1: Nether, 2: The End).
    pub dimension: VarI32,
    /// Generator type (1: Flat, 2: Infinite).
    pub generator: VarI32,
    /// World default gamemode.
    pub world_gamemode: VarI32,
    /// World difficulty.
    pub difficulty: VarI32,
    /// Default spawn X coordinate.
    pub spawn_x: VarI32,
    /// Default spawn Y coordinate.
    pub spawn_y: VarU32,
    /// Default spawn Z coordinate.
    pub spawn_z: VarI32,
    /// True if achievements are disabled (e.g. cheats on).
    pub has_achievements_disabled: bool,
    /// Name of the world level.
    pub world_name: String,
}

impl BedrockEncode for StartGame {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        write_packet_header(packet_ids::START_GAME, w)?;
        self.entity_id.encode(w)?;
        self.runtime_entity_id.encode(w)?;
        self.player_gamemode.encode(w)?;
        self.player_position.x.encode(w)?;
        self.player_position.y.encode(w)?;
        self.player_position.z.encode(w)?;
        self.pitch.encode(w)?;
        self.yaw.encode(w)?;
        self.seed.encode(w)?;
        self.dimension.encode(w)?;
        self.generator.encode(w)?;
        self.world_gamemode.encode(w)?;
        self.difficulty.encode(w)?;
        self.spawn_x.encode(w)?;
        self.spawn_y.encode(w)?;
        self.spawn_z.encode(w)?;
        self.has_achievements_disabled.encode(w)?;
        self.world_name.encode(w)?;
        Ok(())
    }
}

impl<'a> BedrockDecode<'a> for StartGame {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        read_packet_header(r, packet_ids::START_GAME)?;
        let entity_id = VarI64::decode(r)?;
        let runtime_entity_id = VarU64::decode(r)?;
        let player_gamemode = VarI32::decode(r)?;
        let x = f32::decode(r)?;
        let y = f32::decode(r)?;
        let z = f32::decode(r)?;
        let pitch = f32::decode(r)?;
        let yaw = f32::decode(r)?;
        let seed = i64::decode(r)?;
        let dimension = VarI32::decode(r)?;
        let generator = VarI32::decode(r)?;
        let world_gamemode = VarI32::decode(r)?;
        let difficulty = VarI32::decode(r)?;
        let spawn_x = VarI32::decode(r)?;
        let spawn_y = VarU32::decode(r)?;
        let spawn_z = VarI32::decode(r)?;
        let has_achievements_disabled = bool::decode(r)?;
        let world_name = String::decode(r)?;

        Ok(Self {
            entity_id,
            runtime_entity_id,
            player_gamemode,
            player_position: Vec3::new(x, y, z),
            pitch,
            yaw,
            seed,
            dimension,
            generator,
            world_gamemode,
            difficulty,
            spawn_x,
            spawn_y,
            spawn_z,
            has_achievements_disabled,
            world_name,
        })
    }
}
