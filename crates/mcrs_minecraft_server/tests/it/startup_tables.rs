use bevy_app::App;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::light::BlockLightRegistry;
use mcrs_minecraft_dimension::environment::DimensionEnvironments;
use mcrs_minecraft_environment::world_clock::WorldClocks;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_server::{Lighting, MinecraftServerPlugin};
use mcrs_minecraft_worldgen_generator::heightmap::HeightmapPredicates;

#[test]
fn the_startup_tables_and_clocks_exist_after_the_first_update() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins(MinecraftServerPlugin {
        asset_path: Some(assets.to_str().unwrap().to_owned()),
        lighting: Lighting::Propagated,
        announce_on_lan: false,
        ..MinecraftServerPlugin::embedded()
    });
    app.finish();
    app.cleanup();
    app.update();

    assert_ne!(
        *app.world().resource::<State<AppState>>().get(),
        AppState::Playing,
        "the update must not have reached Playing, or it proves nothing about Startup"
    );
    assert!(
        app.world().get_resource::<BlockLightRegistry>().is_some(),
        "no block light table after the first update"
    );
    assert!(
        app.world().get_resource::<HeightmapPredicates>().is_some(),
        "no heightmap predicates after the first update"
    );
    let environments = app.world().resource::<DimensionEnvironments>();
    assert!(
        !environments.is_empty(),
        "no dimension environments after the first update"
    );
    let clocks = app.world().resource::<WorldClocks>();
    let registered = app
        .world()
        .resource::<RegistrySet>()
        .registry::<keys::WorldClock>()
        .expect("the world clock registry is loaded");
    assert!(!registered.is_empty());
    assert_eq!(
        clocks.len(),
        registered.len(),
        "the clocks held after the first update differ from the registry's"
    );
    for id in registered.ids() {
        assert!(clocks.get(id).is_some(), "{id:?} is not seeded");
    }
}
