use bevy::app::{TaskPoolOptions, TaskPoolPlugin};
use bevy::asset::io::AssetSourceId;
use bevy::prelude::*;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_client::registries::ClientRegistriesPlugin;
use mcrs_minecraft_client::{asset_corpus, asset_source};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_dimension_environment::environment::EnvironmentAttributes;
use mcrs_minecraft_network::client::SessionRegistryInputs;
use mcrs_minecraft_registry::{NetworkRegistry, RegistrySet};
use mcrs_minecraft_world::registries::{
    registries_as_sent_to_a_client_that_knows_vanilla, tags_as_sent, test_registries,
};

pub fn boot(extra: impl FnOnce(&mut App)) -> App {
    let assets = asset_corpus().to_string_lossy().into_owned();
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: TaskPoolOptions::with_num_threads(2),
    })
    .register_asset_source(AssetSourceId::Default, asset_source(&assets));
    app.add_plugins(AssetPlugin {
        file_path: assets,
        watch_for_changes_override: Some(false),
        ..default()
    })
    .add_plugins(ClientRegistriesPlugin::default());
    extra(&mut app);
    app.finish();
    app.cleanup();
    app
}

/// What the client builds when a server holding the local corpus finishes configuring: the
/// vanilla entries arrive without data and are filled from the known pack.
pub fn session_registries(app: &App, edit: impl FnOnce(&mut Vec<NetworkRegistry>)) -> RegistrySet {
    let inputs = app.world().resource::<SessionRegistryInputs>();
    let server = test_registries();
    let mut sent = registries_as_sent_to_a_client_that_knows_vanilla(server);
    edit(&mut sent);
    let tags = tags_as_sent(&inputs.statics, server);
    inputs
        .declarations
        .from_network(&inputs.statics, sent, &tags, inputs.known.as_ref())
        .unwrap_or_else(|report| panic!("{report}"))
}

#[test]
fn the_client_boots_without_local_world_registries() {
    let app = boot(|_| {});

    assert!(
        app.world().get_resource::<RegistrySet>().is_none(),
        "world registries are session data, so none exists before a server sends them"
    );
    let blocks = app.world().resource::<Blocks>();
    assert!(blocks.state_count() > 0, "block definitions load at start");

    let inputs = app.world().resource::<SessionRegistryInputs>();
    assert!(
        inputs.statics.registry::<Block>().is_some(),
        "the static registries are built at start"
    );
    assert!(
        inputs.declarations.synced().next().is_some(),
        "the declarations name the registries a server syncs"
    );
    assert!(
        inputs.known.as_ref().is_some_and(|known| !known.is_empty()),
        "a native client holds the vanilla pack's entries to fill what a server leaves out"
    );
}

#[test]
fn every_dimension_type_of_a_filled_session_has_its_environment() {
    let app = boot(|_| {});
    let registries = session_registries(&app, |_| {});

    let types = registries
        .registry::<DimensionType>()
        .expect("the dimension type registry is received");
    let received = registries
        .entries::<DimensionType, DimensionType>()
        .expect("the dimension type column is received");
    assert!(!types.is_empty());
    assert_eq!(received.as_slice().len(), types.len());

    for id in types.ids() {
        let name = types.name(id).expect("an id of the registry has a name");
        let dimension = ResourceKey::<Dimension>::from_location(name.clone());
        assert!(
            EnvironmentAttributes::of_dimension(&registries, &dimension, id).is_ok(),
            "every dimension type has the environment its sky is built from: {name}"
        );
    }
}
