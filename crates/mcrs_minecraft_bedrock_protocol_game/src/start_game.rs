use alloc::vec::Vec;

use glam::{Vec2, Vec3};
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32, VarI64, VarU64};

use crate::block_property::BlockProperty;
use crate::item_table_entry::ItemTableEntry;
use crate::level_settings::LevelSettings;
use crate::movement_settings::MovementSettings;
use crate::network_nbt::NetworkNbt;
use crate::world_template_id::WorldTemplateId;

#[derive(Clone, Debug, PartialEq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x0b)]
pub struct StartGame<'a> {
    pub entity_unique_id: VarI64,
    pub entity_runtime_id: VarU64,
    pub player_game_type: PlayerGameType,
    pub position: Vec3,
    pub rotation: Vec2,
    pub level_settings: LevelSettings<'a>,
    pub level_id: &'a str,
    pub level_name: &'a str,
    pub template_content_identity: &'a str,
    pub trial: bool,
    pub movement_settings: MovementSettings,
    pub current_level_time: i64,
    pub enchantment_seed: VarI32,
    pub block_properties: Vec<BlockProperty<'a>>,
    pub item_table: Vec<ItemTableEntry<'a>>,
    pub multiplayer_correlation_id: &'a str,
    pub item_stack_net_manager: bool,
    pub server_version: &'a str,
    pub player_property_data: NetworkNbt,
    pub block_registry_checksum: u64,
    pub world_template_id: WorldTemplateId,
    pub client_side_generation: bool,
    pub block_network_ids_are_hashes: bool,
    pub server_authoritative_sound: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum PlayerGameType {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Default = 5,
    Spectator = 6,
}
