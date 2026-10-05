use bevy_app::App;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::light::BlockLightRegistry;
use mcrs_minecraft_environment::world_clock::WorldClocks;
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_server::{Lighting, MinecraftServerPlugin};

#[test]
fn the_light_table_and_clocks_exist_after_the_first_update() {
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
    let clocks = app.world().resource::<WorldClocks>();
    let registered = app
        .world()
        .resource::<RegistrySet>()
        .table("minecraft:world_clock")
        .expect("the world clock registry is loaded");
    assert!(!registered.is_empty());
    assert_eq!(
        clocks.len(),
        registered.len(),
        "the clocks held after the first update differ from the registry's"
    );
    for name in registered.names() {
        assert!(clocks.get(name.as_str()).is_some(), "{name} is not seeded");
    }
}
