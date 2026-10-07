use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::prelude::{
    Changed, Commands, IntoScheduleConfigs, Query, Res, Resource, resource_exists,
};
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientNetworkSystems, ReceivedRegistries};
use mcrs_minecraft_registry::{Id, Registered, RegistrySet};

/// What the server's registry numbers mean in the local registries, mapped by name when
/// configuration ends and replaced whole when it is entered again. `Id<R>` has no public
/// constructor from a number, so this is the only way a received number becomes one.
#[derive(Resource, Clone, Default)]
pub struct WireIds {
    pub biomes: Option<WireTable<Biome>>,
    pub dimension_types: Option<WireTable<DimensionType>>,
}

impl WireIds {
    pub fn build(received: &ReceivedRegistries, local: &RegistrySet) -> Self {
        WireIds {
            biomes: WireTable::build(received, local),
            dimension_types: WireTable::build(received, local),
        }
    }
}

pub struct WireTable<R>(Arc<[Option<Id<R>>]>);

impl<R> Clone for WireTable<R> {
    fn clone(&self) -> Self {
        WireTable(Arc::clone(&self.0))
    }
}

impl<R: Registered> WireTable<R> {
    fn build(received: &ReceivedRegistries, local: &RegistrySet) -> Option<Self> {
        let registry = local.registry::<R>()?;
        let sent = received
            .0
            .iter()
            .find(|sent| sent.registry == R::REGISTRY.location().as_static_str())?;
        Some(WireTable(
            sent.entries
                .iter()
                .map(|entry| registry.by_name(&entry.id))
                .collect(),
        ))
    }
}

impl<R> WireTable<R> {
    pub fn get(&self, number: u16) -> Option<Id<R>> {
        self.0.get(usize::from(number)).copied()?
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub struct WireIdPlugin;

impl Plugin for WireIdPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            rebuild_wire_ids
                .after(ClientNetworkSystems::Receive)
                .run_if(resource_exists::<RegistrySet>),
        );
    }
}

pub(crate) fn rebuild_wire_ids(
    connections: Query<(&ConnectionState, &ReceivedRegistries), Changed<ConnectionState>>,
    local: Res<RegistrySet>,
    mut commands: Commands,
) {
    for (state, received) in &connections {
        match state {
            ConnectionState::Game => commands.insert_resource(WireIds::build(received, &local)),
            _ => commands.remove_resource::<WireIds>(),
        }
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use mcrs_minecraft_core::ResourceLocation;
    use mcrs_minecraft_network::client::{ReceivedRegistry, RegistryEntry};
    use mcrs_minecraft_registry::Registry;

    pub fn local_biomes(names: &[&str]) -> RegistrySet {
        let names = names
            .iter()
            .map(|name| ResourceLocation::<Arc<str>>::read(name).unwrap());
        RegistrySet::new()
            .with(
                Registry::<mcrs_minecraft_biome::Biome>::new(
                    mcrs_minecraft_biome::keys::BIOME,
                    names,
                )
                .unwrap(),
            )
            .unwrap()
    }

    pub fn received_biomes(names: &[&str]) -> ReceivedRegistries {
        let mut received = ReceivedRegistries::default();
        received.push(ReceivedRegistry {
            registry: mcrs_minecraft_biome::keys::BIOME
                .location()
                .as_static_str()
                .to_owned(),
            entries: names
                .iter()
                .map(|name| RegistryEntry {
                    id: (*name).to_owned(),
                    data: None,
                })
                .collect(),
        });
        received
    }

    pub fn biome_names(len: usize) -> Vec<String> {
        (0..len).map(|n| format!("minecraft:b{n}")).collect()
    }

    pub fn wire_ids_of(local: &[String], server: &[String]) -> WireIds {
        fn borrowed(names: &[String]) -> Vec<&str> {
            names.iter().map(String::as_str).collect()
        }
        WireIds::build(
            &received_biomes(&borrowed(server)),
            &local_biomes(&borrowed(local)),
        )
    }

    pub fn identity_biomes(len: usize) -> WireIds {
        let names = biome_names(len);
        wire_ids_of(&names, &names)
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn a_received_number_maps_to_a_local_id_only_through_the_session_table() {
        let local = local_biomes(&["minecraft:a", "minecraft:b"]);
        let received = received_biomes(&["minecraft:b", "minecraft:a", "minecraft:gone"]);
        let ids = WireIds::build(&received, &local);
        let biomes = local.registry::<Biome>().unwrap();

        let table = ids.biomes.unwrap();
        assert!(biomes.by_name("minecraft:b").is_some());
        assert_eq!(table.get(0), biomes.by_name("minecraft:b"));
        assert_eq!(table.get(1), biomes.by_name("minecraft:a"));
        assert_eq!(table.get(2), None, "a name the local set lacks");
        assert_eq!(table.get(3), None, "past the list the server sent");
        assert_eq!(table.len(), 3);
    }

    #[test]
    fn the_table_is_built_when_configuration_ends_and_dropped_when_it_starts_again() {
        let mut app = App::new();
        app.insert_resource(local_biomes(&["minecraft:a"]))
            .add_systems(Update, rebuild_wire_ids);
        let connection = app
            .world_mut()
            .spawn((
                ConnectionState::Configuration,
                received_biomes(&["minecraft:a"]),
            ))
            .id();
        let enter = |app: &mut App, state| {
            *app.world_mut()
                .get_mut::<ConnectionState>(connection)
                .unwrap() = state;
            app.update();
        };

        app.update();
        assert!(!app.world().contains_resource::<WireIds>());

        enter(&mut app, ConnectionState::Game);
        let ids = app.world().resource::<WireIds>();
        assert!(ids.biomes.as_ref().and_then(|table| table.get(0)).is_some());

        enter(&mut app, ConnectionState::Configuration);
        assert!(!app.world().contains_resource::<WireIds>());
    }

    #[test]
    fn a_registry_the_server_never_sent_has_no_table() {
        let ids = WireIds::build(
            &ReceivedRegistries::default(),
            &local_biomes(&["minecraft:a"]),
        );
        assert!(ids.biomes.is_none());
    }
}
