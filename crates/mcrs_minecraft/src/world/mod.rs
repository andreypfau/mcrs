use crate::world::block::minecraft::MinecraftBlockPlugin;
use crate::world::block_update::BlockUpdatePlugin;
use crate::world::chunk::ChunkPlugin;
use crate::world::entity::MinecraftEntityPlugin;
use crate::world::explosion::ExplosionPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use mcrs_engine::world::dimension::DimensionPlugin;
use mcrs_protocol::WritePacket;

pub mod block;
mod block_update;
pub mod chunk;
pub mod entity;
mod explosion;
mod format;
mod generate;
mod inventory;
pub mod item;
mod material;
mod palette;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(DimensionPlugin);
        app.add_plugins(ChunkPlugin);
        app.add_plugins(BlockUpdatePlugin);
        app.add_plugins(MinecraftEntityPlugin);
        app.add_plugins(MinecraftBlockPlugin);
        app.add_plugins(ExplosionPlugin);
    }
}
