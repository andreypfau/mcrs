use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_registries::CurrentRegistries;

#[derive(Resource, Debug)]
pub struct PhysicsTables(Tables);

impl PhysicsTables {
    #[must_use]
    pub fn new(tables: Tables) -> Self {
        Self(tables)
    }

    #[must_use]
    pub fn tables(&self) -> &Tables {
        &self.0
    }

    #[must_use]
    pub fn collision(&self) -> &CollisionTable {
        &self.0.collision
    }
}

pub(crate) fn build_tables(
    mut commands: Commands,
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
) {
    let Some(tags) = registries.tags::<Block>() else {
        tracing::error!("the registries declare no block tags; entities keep their tables");
        return;
    };
    commands.insert_resource(PhysicsTables::new(Tables::new(collision_table(
        &blocks, &tags,
    ))));
}
