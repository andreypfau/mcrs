use bevy_ecs::prelude::*;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_registries::CurrentRegistries;

#[derive(Resource)]
pub(crate) struct ClientTables(pub(crate) Tables);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::per_frame_insert_resource,
        reason = "the system runs only when the registries change, and the new tables replace the old ones"
    )
)]
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

pub(crate) fn clear_client_tables(mut commands: Commands) {
    commands.remove_resource::<ClientTables>();
}
