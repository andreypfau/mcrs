use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::ecs::prelude::{
    Changed, Commands, IntoScheduleConfigs, Query, Res, Resource, resource_exists,
};
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientNetworkSystems, ReceivedRegistries};
use mcrs_minecraft_registry::{Id, RegistrySet};

pub struct WireId<R> {
    number: u16,
    _marker: PhantomData<fn() -> R>,
}

impl<R> WireId<R> {
    pub fn received(number: u16) -> Self {
        WireId {
            number,
            _marker: PhantomData,
        }
    }

    pub fn number(self) -> u16 {
        self.number
    }
}

impl<R> Clone for WireId<R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<R> Copy for WireId<R> {}
impl<R> PartialEq for WireId<R> {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number
    }
}
impl<R> Eq for WireId<R> {}
impl<R> Hash for WireId<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.number.hash(state);
    }
}
impl<R> fmt::Debug for WireId<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WireId({})", self.number)
    }
}

type Table<R> = Box<[Option<Id<R>>]>;

/// What the server's registry numbers mean in the local registries, mapped by name when
/// configuration ends and replaced whole when it is entered again. `Id<R>` has no public
/// constructor from a number, so this is the only way a received number becomes one.
#[derive(Resource, Clone, Default)]
pub struct WireIds {
    tables: Arc<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl WireIds {
    pub fn build(received: &ReceivedRegistries, local: &RegistrySet) -> Self {
        let mut tables = HashMap::new();
        // chisle: only the registries whose numbers the client reads; one more is a line here
        // when something starts decoding its numbers.
        insert::<mcrs_minecraft_biome::Biome>(&mut tables, received, local);
        insert::<DimensionType>(&mut tables, received, local);
        WireIds {
            tables: Arc::new(tables),
        }
    }

    pub fn get<R: mcrs_minecraft_registry::Registered>(&self, wire: WireId<R>) -> Option<Id<R>> {
        self.table::<R>()?.get(usize::from(wire.number)).copied()?
    }

    pub fn sent_len<R: mcrs_minecraft_registry::Registered>(&self) -> Option<usize> {
        Some(self.table::<R>()?.len())
    }

    fn table<R: mcrs_minecraft_registry::Registered>(&self) -> Option<&Table<R>> {
        self.tables.get(&TypeId::of::<R>())?.downcast_ref()
    }
}

fn insert<R: mcrs_minecraft_registry::Registered>(
    tables: &mut HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    received: &ReceivedRegistries,
    local: &RegistrySet,
) {
    let Some(registry) = local.registry::<R>() else {
        return;
    };
    let Some(sent) = received
        .0
        .iter()
        .find(|sent| sent.registry == R::REGISTRY.location().as_static_str())
    else {
        return;
    };
    let table: Table<R> = sent
        .entries
        .iter()
        .map(|entry| registry.by_name(&entry.id))
        .collect();
    tables.insert(TypeId::of::<R>(), Box::new(table));
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
        let biomes = local.registry::<mcrs_minecraft_biome::Biome>().unwrap();
        let wire = WireId::<mcrs_minecraft_biome::Biome>::received;

        assert!(biomes.by_name("minecraft:b").is_some());
        assert_eq!(ids.get(wire(0)), biomes.by_name("minecraft:b"));
        assert_eq!(ids.get(wire(1)), biomes.by_name("minecraft:a"));
        assert_eq!(ids.get(wire(2)), None, "a name the local set lacks");
        assert_eq!(ids.get(wire(3)), None, "past the list the server sent");
        assert_eq!(ids.sent_len::<mcrs_minecraft_biome::Biome>(), Some(3));
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
        assert!(
            ids.get(WireId::<mcrs_minecraft_biome::Biome>::received(0))
                .is_some()
        );

        enter(&mut app, ConnectionState::Configuration);
        assert!(!app.world().contains_resource::<WireIds>());
    }

    #[test]
    fn a_registry_the_server_never_sent_has_no_table() {
        let ids = WireIds::build(
            &ReceivedRegistries::default(),
            &local_biomes(&["minecraft:a"]),
        );
        assert_eq!(ids.sent_len::<mcrs_minecraft_biome::Biome>(), None);
        assert_eq!(
            ids.get(WireId::<mcrs_minecraft_biome::Biome>::received(0)),
            None
        );
    }
}
