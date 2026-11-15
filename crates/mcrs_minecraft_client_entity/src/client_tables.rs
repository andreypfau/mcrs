use bevy_ecs::prelude::*;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_registries::CurrentRegistries;

/// The tables of the physics kernel, built from the block definitions and the block tags of the
/// session.
#[derive(Resource)]
pub(crate) struct ClientTables(pub(crate) Tables);

pub(crate) fn build_client_tables(
    mut commands: Commands,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
) {
    let Some(tags) = registries.tags::<Block>() else {
        tracing::error!(
            "the registries declare no block tags; simulated entities keep their tables"
        );
        return;
    };
    commands.insert_resource(ClientTables(Tables::new(collision_table(&blocks, &tags))));
}
