use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::asset::io::AssetSourceId;
use bevy::prelude::*;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_client::asset_source;
use mcrs_minecraft_client::registries::ClientRegistriesPlugin;
use mcrs_minecraft_core::{Id, ResourceKey, ResourceLocation};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::EnvironmentAttributes;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{HoldsVanillaPack, RegistriesReceived, ServerRegistries};
use mcrs_minecraft_registries::{CurrentRegistries, Registries, RegistryOf};
use mcrs_minecraft_world::registry_load::{Collected, sent_by, test_packs, test_registries_arc};
use std::collections::BTreeMap;

pub fn boot(extra: impl FnOnce(&mut App)) -> App {
    let assets = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_owned();

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: TaskPoolOptions::with_num_threads(2),
    })
    .register_asset_source(
        AssetSourceId::Default,
        asset_source(&mcrs_minecraft_assets::packs::PackRoots {
            assets: assets.clone(),
            vanilla: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
        }),
    );
    app.add_plugins(AssetPlugin {
        file_path: assets,
        watch_for_changes_override: Some(false),
        ..default()
    })
    .add_plugins(ClientRegistriesPlugin {
        local_corpus: Some(test_packs()),
    });
    extra(&mut app);
    app.finish();
    app.cleanup();
    app
}

/// Hands the client what a server holding the local corpus sends at the end of configuration:
/// the vanilla entries arrive without data. `edit` changes it on the way.
pub fn receive_registries(app: &mut App, connection: Entity, edit: impl FnOnce(&mut Collected)) {
    let mut sent = sent_by(test_registries_arc(), true);
    edit(&mut sent);
    let (registries, tags) = sent;
    app.world_mut().trigger(RegistriesReceived {
        entity: connection,
        registries: ServerRegistries {
            registries,
            tags,
            vanilla_known: true,
        },
    });
    app.world_mut().flush();
}

/// The number `name` has in the received registry of `V`, as the id type `R` that names it.
pub fn received_id<V: Send + Sync + 'static, R>(app: &App, name: &str) -> Id<R> {
    let registry = app
        .world()
        .resource::<CurrentRegistries>()
        .registry::<RegistryOf<V>>()
        .expect("the registry is received");
    registry
        .by_name(&ResourceLocation::read(name).unwrap())
        .and_then(|id| Id::checked(id.number(), registry.len()))
        .unwrap_or_else(|| panic!("{name} is not received"))
}

type Contents = BTreeMap<String, (Vec<String>, Vec<(String, Vec<u16>)>)>;

fn contents(set: &Registries, registries: &[String]) -> Contents {
    registries
        .iter()
        .map(|registry| {
            let key = ResourceLocation::read(registry).unwrap();
            let names = set
                .names_of(&key)
                .map(|names| names.iter().map(ToString::to_string).collect())
                .unwrap_or_default();
            let tags = set
                .tags_of(&key)
                .map(|tags| {
                    tags.map(|(name, members)| (name.to_string(), members.to_vec()))
                        .collect()
                })
                .unwrap_or_default();
            (registry.clone(), (names, tags))
        })
        .collect()
}

#[test]
fn a_client_ends_configuration_with_the_registries_the_server_holds() {
    let mut app = boot(|_| {});
    assert!(
        app.world().get_resource::<CurrentRegistries>().is_none(),
        "world registries are session data, so none exists before a server sends them"
    );
    assert!(app.world().resource::<Blocks>().state_count() > 0);
    assert!(app.world().contains_resource::<HoldsVanillaPack>());

    let connection = app.world_mut().spawn(ConnectionState::Configuration).id();
    receive_registries(&mut app, connection, |_| {});

    assert_eq!(
        app.world().get::<ConnectionState>(connection),
        Some(&ConnectionState::Game)
    );
    let (registries, tags) = sent_by(test_registries_arc(), true);
    let sent: Vec<String> = registries
        .iter()
        .map(|(key, _)| key.to_string())
        .chain(tags.iter().map(|(key, _)| key.to_string()))
        .collect();
    assert_eq!(
        contents(app.world().resource::<CurrentRegistries>(), &sent),
        contents(test_registries_arc(), &sent)
    );

    let registries = app.world().resource::<CurrentRegistries>();
    let types = registries
        .registry::<RegistryOf<DimensionTypeFile>>()
        .expect("the dimension type registry is received");
    assert!(!types.is_empty());
    for (id, name, _) in types.iter() {
        let dimension = ResourceKey::<Dimension>::from_location(name.clone());
        let id = Id::checked(id.number(), types.len()).unwrap();
        assert!(
            EnvironmentAttributes::of_dimension(registries, &dimension, id).is_ok(),
            "every dimension type has the environment its sky is built from: {name}"
        );
    }
}
