use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Experiment, NetworkBlockPos, VarI32,
};

use crate::dimension_id::DimensionId;
use crate::game_rule::GameRule;

#[derive(Clone, Debug, PartialEq, BedrockEncode, BedrockDecode)]
pub struct LevelSettings<'a> {
    pub seed: u64,
    pub spawn_biome_type: SpawnBiomeType,
    pub user_defined_biome_name: &'a str,
    pub dimension: DimensionId,
    pub generator: Generator,
    pub world_game_type: GameType,
    pub hardcore: bool,
    pub difficulty: Difficulty,
    pub world_spawn: NetworkBlockPos,
    pub achievements_disabled: bool,
    pub editor_world_type: EditorWorldType,
    pub created_in_editor: bool,
    pub exported_from_editor: bool,
    pub day_cycle_stop_time: VarI32,
    pub education_offer: EducationOffer,
    pub education_features_enabled: bool,
    pub education_product_id: &'a str,
    pub rain_level: f32,
    pub lightning_level: f32,
    pub confirmed_platform_locked_content: bool,
    pub multiplayer_game_intent: bool,
    pub lan_broadcast_intent: bool,
    pub xbox_live_broadcast_mode: BroadcastMode,
    pub platform_broadcast_mode: BroadcastMode,
    pub commands_enabled: bool,
    pub texture_packs_required: bool,
    pub game_rules: Vec<GameRule<'a>>,
    #[bedrock(count = u32)]
    pub experiments: Vec<Experiment<'a>>,
    pub experiments_previously_toggled: bool,
    pub bonus_chest: bool,
    pub start_with_map: bool,
    pub player_permission: PlayerPermission,
    pub server_chunk_tick_range: i32,
    pub locked_behaviour_pack: bool,
    pub locked_resource_pack: bool,
    pub from_locked_template: bool,
    pub only_microsoft_account_gamertags: bool,
    pub from_world_template: bool,
    pub world_template_option_locked: bool,
    pub only_old_villagers: bool,
    pub persona_disabled: bool,
    pub custom_skins_disabled: bool,
    pub emote_chat_muted: bool,
    pub base_game_version: &'a str,
    pub limited_world_width: i32,
    pub limited_world_depth: i32,
    pub new_nether: bool,
    pub education_shared_resource_button_name: &'a str,
    pub education_shared_resource_link: &'a str,
    pub force_experimental_gameplay: Option<bool>,
    pub chat_restriction_level: ChatRestrictionLevel,
    pub player_interactions_disabled: bool,
    pub server_id: &'a str,
    pub world_id: &'a str,
    pub scenario_id: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = i16)]
pub enum SpawnBiomeType {
    Default = 0,
    UserDefined = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum Generator {
    Legacy = 0,
    Overworld = 1,
    Flat = 2,
    Nether = 3,
    End = 4,
    Void = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum GameType {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
pub enum Difficulty {
    Peaceful = 0,
    Easy = 1,
    Normal = 2,
    Hard = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum EditorWorldType {
    NotEditor = 0,
    Project = 1,
    TestLevel = 2,
    RealmsUpload = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum EducationOffer {
    None = 0,
    RestOfWorld = 1,
    China = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum BroadcastMode {
    None = 0,
    InviteOnly = 1,
    FriendsOnly = 2,
    FriendsOfFriends = 3,
    Public = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum PlayerPermission {
    Visitor = 0,
    Member = 1,
    Operator = 2,
    Custom = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = u8)]
#[non_exhaustive]
pub enum ChatRestrictionLevel {
    None = 0,
    Dropped = 1,
    Disabled = 2,
}
