use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::block_update::BlockUpdatePlugin;
use crate::world::chunk::ChunkPlugin;
use crate::world::entity::MinecraftEntityPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_engine::world::dimension::DimensionPlugin;
use mcrs_protocol::WritePacket;

pub mod block;
mod block_update;
pub mod chunk;
pub mod chunk_observer;
mod chunk_tickets;
pub mod entity;
mod format;
mod generate;
mod palette;
mod player_chunk_loader;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DimensionPlugin);
        app.add_plugins(ChunkPlugin);
        app.add_plugins(BlockUpdatePlugin);
        app.add_plugins(MinecraftEntityPlugin);
        app.add_plugins(MinecraftBlockPlugin);
    }
}
