use bevy::log::error;
use bevy::prelude::*;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_collision_table::collision_table;
use mcrs_minecraft_physics::Tables;
use mcrs_minecraft_registries::CurrentRegistries;

#[derive(Resource, Debug)]
pub struct ClientPhysicsTables(pub(crate) Tables);

impl ClientPhysicsTables {
    #[must_use]
    pub fn tables(&self) -> &Tables {
        &self.0
    }
}

pub(crate) fn build_collision_tables(
    blocks: Res<Blocks>,
    registries: Res<CurrentRegistries>,
    mut commands: Commands,
) {
    let Some(tags) = registries.tags::<Block>() else {
        error!("the session registries declare no block tags to build the collision tables from");
        commands.remove_resource::<ClientPhysicsTables>();
        return;
    };
    commands.insert_resource(ClientPhysicsTables(Tables::new(collision_table(
        &blocks, &tags,
    ))));
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;
    use bevy::prelude::*;
    use mcrs_minecraft_block::definition::Blocks;
    use mcrs_minecraft_game_registries::registry_load::test_registries;
    use mcrs_minecraft_network::ConnectionState;
    use mcrs_minecraft_registries::CurrentRegistries;

    use super::ClientPhysicsTables;
    use crate::registries::ClientRegistriesPlugin;

    #[test]
    fn the_collision_tables_follow_the_session_registries() {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            },
            ClientRegistriesPlugin::default(),
        ));
        app.finish();
        app.cleanup();
        app.insert_resource(CurrentRegistries::new(test_registries().clone()));
        app.update();

        let blocks = app.world().resource::<Blocks>();
        let stone = blocks.default_state("minecraft:stone").unwrap();
        let air = blocks.default_state("minecraft:air").unwrap();
        let tables = app.world().resource::<ClientPhysicsTables>().tables();
        assert!(tables.collision.state(stone).is_solid());
        assert!(!tables.collision.state(air).is_solid());

        app.world_mut().spawn(ConnectionState::Configuration);
        app.update();
        assert!(app.world().get_resource::<ClientPhysicsTables>().is_none());
    }
}
