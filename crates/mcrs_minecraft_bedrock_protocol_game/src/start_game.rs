use alloc::vec::Vec;

use glam::{Vec2, Vec3};
use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, VarI32, VarI64, VarU64};

use crate::block_property::BlockProperty;
use crate::item_table_entry::ItemTableEntry;
use crate::level_settings::LevelSettings;
use crate::movement_settings::MovementSettings;
use crate::network_nbt::NetworkNbt;
use crate::world_template_id::WorldTemplateId;

/// Starts the world for the player: the identity of the player, the world settings and the tables
/// of the connection.
#[derive(Clone, Debug, PartialEq, BedrockEncode, BedrockDecode)]
#[bedrock(id = 0x0b)]
pub struct StartGame<'a> {
    /// The unique id of the player.
    pub entity_unique_id: VarI64,
    /// The runtime id of the player.
    pub entity_runtime_id: VarU64,
    /// The game type of the player.
    pub player_game_type: PlayerGameType,
    /// The eye position of the player.
    pub position: Vec3,
    /// The pitch as x and the yaw as y, in degrees.
    pub rotation: Vec2,
    /// The settings of the world.
    pub level_settings: LevelSettings<'a>,
    /// An opaque id of the world.
    pub level_id: &'a str,
    /// The name that the pause screen shows.
    pub level_name: &'a str,
    /// The template content identity; empty when the world is not from a template.
    pub template_content_identity: &'a str,
    /// Whether the world is a trial.
    pub trial: bool,
    /// The movement settings.
    pub movement_settings: MovementSettings,
    /// The time of day in ticks.
    pub current_level_time: i64,
    /// The enchantment seed.
    pub enchantment_seed: VarI32,
    /// The custom blocks; empty when the server has none.
    pub block_properties: Vec<BlockProperty<'a>>,
    /// The item network ids of the connection.
    pub item_table: Vec<ItemTableEntry<'a>>,
    /// The multiplayer correlation id.
    pub multiplayer_correlation_id: &'a str,
    /// Whether the client sends item stack requests.
    pub item_stack_net_manager: bool,
    /// A free text, for example `mcrs`.
    pub server_version: &'a str,
    /// The player properties; an empty compound when the server has none.
    pub player_property_data: NetworkNbt,
    /// The block registry checksum; a server of mcrs writes 0, and the client then does not
    /// compare the block registry.
    pub block_registry_checksum: u64,
    /// The world template; the zero UUID when the world is not from a template.
    pub world_template_id: WorldTemplateId,
    /// Whether the client generates the world.
    pub client_side_generation: bool,
    /// Whether block runtime ids are hashes; a server of mcrs writes false, and the ids are then
    /// the indices of the block palette.
    pub block_network_ids_are_hashes: bool,
    /// Whether the server owns the sounds.
    pub server_authoritative_sound: bool,
}

/// The game type of the player in [`StartGame`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum PlayerGameType {
    /// Survival.
    Survival = 0,
    /// Creative.
    Creative = 1,
    /// Adventure.
    Adventure = 2,
    /// The game type of the world, [`LevelSettings::world_game_type`].
    Default = 5,
    /// Spectator.
    Spectator = 6,
}
