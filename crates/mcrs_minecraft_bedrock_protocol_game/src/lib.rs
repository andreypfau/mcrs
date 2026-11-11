//! Bedrock Edition game packets: StartGame, Text, MovePlayer, PlayerAuthInput, SetTime, UpdateBlock and the chunk column packet.
// std: mcrs_minecraft_nbt

extern crate alloc;

mod block_action;
mod block_property;
mod chunk_column;
mod dimension_id;
mod game_rule;
mod input_flags;
mod inventory_action;
mod item_descriptor;
mod item_stack;
mod item_stack_request;
mod item_table_entry;
mod item_use_transaction;
mod level_settings;
mod move_player;
mod movement_settings;
mod network_nbt;
mod player_auth_input;
mod set_time;
mod start_game;
mod text;
mod update_block;
mod world_template_id;

pub use block_action::{BlockAction, TargetedBlockAction, UntargetedBlockAction};
pub use block_property::BlockProperty;
pub use chunk_column::{BlobCache, ChunkColumn, SubChunkCount};
pub use dimension_id::DimensionId;
pub use game_rule::{GameRule, GameRuleValue};
pub use input_flags::InputFlags;
pub use inventory_action::{InventoryAction, InventorySource};
pub use item_descriptor::{DefaultItem, Ingredient, ItemDescriptor};
pub use item_stack::{ItemStack, NetworkItemStack};
pub use item_stack_request::{ItemStackAction, ItemStackRequest, StackSlot};
pub use item_table_entry::ItemTableEntry;
pub use item_use_transaction::{
    InteractPrediction, ItemUseAction, ItemUseTransaction, ItemUseTrigger, LegacyRequest,
    LegacySlots,
};
pub use level_settings::{
    BroadcastMode, ChatRestrictionLevel, Difficulty, EditorWorldType, EducationOffer, GameType,
    Generator, LevelSettings, PlayerPermission, SpawnBiomeType,
};
pub use move_player::{MoveMode, MovePlayer, TeleportCause};
pub use movement_settings::{MovementAuthority, MovementSettings};
pub use network_nbt::NetworkNbt;
pub use player_auth_input::{
    ClientPredictedVehicle, InputMode, InteractionModel, PlayMode, PlayerAuthInput,
};
pub use set_time::SetTime;
pub use start_game::{PlayerGameType, StartGame};
pub use text::{Text, TextContent};
pub use update_block::{BlockLayer, UpdateBlock, UpdateBlockFlags};
pub use world_template_id::WorldTemplateId;
