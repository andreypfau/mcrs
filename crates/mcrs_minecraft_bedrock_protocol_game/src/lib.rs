//! Bedrock Edition game packets: StartGame, Text, MovePlayer, PlayerAuthInput, SetTime, UpdateBlock and the chunk column packet.
#![no_std]

extern crate alloc;

mod block_action;
mod chunk_column;
mod dimension_id;
mod input_flags;
mod inventory_action;
mod item_descriptor;
mod item_stack;
mod item_stack_request;
mod item_use_transaction;
mod move_player;
mod player_auth_input;
mod set_time;
mod start_game;
mod text;
mod update_block;

pub use block_action::{BlockAction, TargetedBlockAction, UntargetedBlockAction};
pub use chunk_column::{BlobCache, ChunkColumn, SubChunkCount};
pub use dimension_id::DimensionId;
pub use input_flags::InputFlags;
pub use inventory_action::{InventoryAction, InventorySource};
pub use item_descriptor::{DefaultItem, Ingredient, ItemDescriptor};
pub use item_stack::{ItemStack, NetworkItemStack};
pub use item_stack_request::{ItemStackAction, ItemStackRequest, StackSlot};
pub use item_use_transaction::{
    InteractPrediction, ItemUseAction, ItemUseTransaction, ItemUseTrigger, LegacyRequest,
    LegacySlots,
};
pub use move_player::{MoveMode, MovePlayer, TeleportCause};
pub use player_auth_input::{
    ClientPredictedVehicle, InputMode, InteractionModel, PlayMode, PlayerAuthInput,
};
pub use set_time::SetTime;
pub use start_game::StartGame;
pub use text::{Text, TextContent};
pub use update_block::{BlockLayer, UpdateBlock, UpdateBlockFlags};
