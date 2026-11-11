use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Experiment, NetworkBlockPos, VarI32,
};

use crate::dimension_id::DimensionId;
use crate::game_rule::GameRule;

/// The settings of the world in [`StartGame`](crate::StartGame).
#[derive(Clone, Debug, PartialEq, BedrockEncode, BedrockDecode)]
pub struct LevelSettings<'a> {
    /// The world seed.
    pub seed: u64,
    /// Whether the spawn biome is the default one or [`Self::user_defined_biome_name`].
    pub spawn_biome_type: SpawnBiomeType,
    /// The spawn biome; empty with [`SpawnBiomeType::Default`].
    pub user_defined_biome_name: &'a str,
    /// The dimension of the player.
    pub dimension: DimensionId,
    /// The world generator.
    pub generator: Generator,
    /// The game type of the world.
    pub world_game_type: GameType,
    /// Whether the world is hardcore.
    pub hardcore: bool,
    /// The difficulty.
    pub difficulty: Difficulty,
    /// The world spawn.
    pub world_spawn: NetworkBlockPos,
    /// Whether achievements are disabled.
    pub achievements_disabled: bool,
    /// The editor world type.
    pub editor_world_type: EditorWorldType,
    /// Whether the world was created in the editor.
    pub created_in_editor: bool,
    /// Whether the world was exported from the editor.
    pub exported_from_editor: bool,
    /// The day cycle stop time.
    pub day_cycle_stop_time: VarI32,
    /// The education offer.
    pub education_offer: EducationOffer,
    /// Whether the education features are on.
    pub education_features_enabled: bool,
    /// The education product id.
    pub education_product_id: &'a str,
    /// The rain level, from 0.0 to 1.0.
    pub rain_level: f32,
    /// The lightning level, from 0.0 to 1.0.
    pub lightning_level: f32,
    /// Whether the platform locked content is confirmed.
    pub confirmed_platform_locked_content: bool,
    /// Whether the world is for multiplayer.
    pub multiplayer_game_intent: bool,
    /// Whether the world is broadcast on the LAN.
    pub lan_broadcast_intent: bool,
    /// The Xbox Live broadcast mode.
    pub xbox_live_broadcast_mode: BroadcastMode,
    /// The platform broadcast mode.
    pub platform_broadcast_mode: BroadcastMode,
    /// Whether commands are on.
    pub commands_enabled: bool,
    /// Whether the texture packs are required.
    pub texture_packs_required: bool,
    /// The game rules.
    pub game_rules: Vec<GameRule<'a>>,
    /// The experiments, behind a `u32le` count.
    #[bedrock(count = u32)]
    pub experiments: Vec<Experiment<'a>>,
    /// Whether experiments were toggled before.
    pub experiments_previously_toggled: bool,
    /// Whether the world has a bonus chest.
    pub bonus_chest: bool,
    /// Whether the player starts with a map.
    pub start_with_map: bool,
    /// The permission of the player.
    pub player_permission: PlayerPermission,
    /// The server chunk tick range in chunks.
    pub server_chunk_tick_range: i32,
    /// Whether the behaviour pack is locked.
    pub locked_behaviour_pack: bool,
    /// Whether the resource pack is locked.
    pub locked_resource_pack: bool,
    /// Whether the world is from a locked template.
    pub from_locked_template: bool,
    /// Whether only Microsoft account gamertags are shown.
    pub only_microsoft_account_gamertags: bool,
    /// Whether the world is from a world template.
    pub from_world_template: bool,
    /// Whether the world template options are locked.
    pub world_template_option_locked: bool,
    /// Whether only old villagers spawn.
    pub only_old_villagers: bool,
    /// Whether personas are disabled.
    pub persona_disabled: bool,
    /// Whether custom skins are disabled.
    pub custom_skins_disabled: bool,
    /// Whether emote chat is muted.
    pub emote_chat_muted: bool,
    /// The base game version; `*` for the newest version.
    pub base_game_version: &'a str,
    /// The limited world width; 0 for an infinite world.
    pub limited_world_width: i32,
    /// The limited world depth; 0 for an infinite world.
    pub limited_world_depth: i32,
    /// Whether the new nether is on.
    pub new_nether: bool,
    /// The name of the education shared resource button.
    pub education_shared_resource_button_name: &'a str,
    /// The link of the education shared resource.
    pub education_shared_resource_link: &'a str,
    /// Whether experimental gameplay is forced, when the server states it.
    pub force_experimental_gameplay: Option<bool>,
    /// The chat restriction level.
    pub chat_restriction_level: ChatRestrictionLevel,
    /// Whether player interactions are disabled.
    pub player_interactions_disabled: bool,
    /// The server id.
    pub server_id: &'a str,
    /// The world id.
    pub world_id: &'a str,
    /// The scenario id.
    pub scenario_id: &'a str,
}

/// The spawn biome type of [`LevelSettings`], as an `i16le`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = i16)]
pub enum SpawnBiomeType {
    /// The default spawn biome.
    Default = 0,
    /// The biome that [`LevelSettings::user_defined_biome_name`] names.
    UserDefined = 1,
}

/// The world generator of [`LevelSettings`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum Generator {
    /// The legacy generator.
    Legacy = 0,
    /// The overworld generator.
    Overworld = 1,
    /// The flat generator.
    Flat = 2,
    /// The nether generator.
    Nether = 3,
    /// The end generator.
    End = 4,
    /// The void generator.
    Void = 5,
}

/// The game type of the world, as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum GameType {
    /// Survival.
    Survival = 0,
    /// Creative.
    Creative = 1,
    /// Adventure.
    Adventure = 2,
    /// Spectator.
    Spectator = 6,
}

/// The difficulty of [`LevelSettings`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
pub enum Difficulty {
    /// Peaceful.
    Peaceful = 0,
    /// Easy.
    Easy = 1,
    /// Normal.
    Normal = 2,
    /// Hard.
    Hard = 3,
}

/// The editor world type of [`LevelSettings`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum EditorWorldType {
    /// Not an editor world.
    NotEditor = 0,
    /// An editor project.
    Project = 1,
    /// A test level.
    TestLevel = 2,
    /// A realms upload.
    RealmsUpload = 3,
}

/// The education offer of [`LevelSettings`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum EducationOffer {
    /// No offer.
    None = 0,
    /// The rest of the world.
    RestOfWorld = 1,
    /// China.
    China = 2,
}

/// A broadcast mode of [`LevelSettings`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum BroadcastMode {
    /// No broadcast.
    None = 0,
    /// Invited players only.
    InviteOnly = 1,
    /// Friends only.
    FriendsOnly = 2,
    /// Friends of friends.
    FriendsOfFriends = 3,
    /// Public.
    Public = 4,
}

/// The permission of the player in [`LevelSettings`], as a `varint32`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = VarI32)]
#[non_exhaustive]
pub enum PlayerPermission {
    /// A visitor.
    Visitor = 0,
    /// A member.
    Member = 1,
    /// An operator.
    Operator = 2,
    /// Custom permissions.
    Custom = 3,
}

/// The chat restriction level of [`LevelSettings`], as a `u8`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BedrockEncode, BedrockDecode)]
#[bedrock(tag = u8)]
#[non_exhaustive]
pub enum ChatRestrictionLevel {
    /// No restriction.
    None = 0,
    /// Chat is dropped.
    Dropped = 1,
    /// Chat is disabled.
    Disabled = 2,
}
