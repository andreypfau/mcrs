use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer, Assets};
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_world::MinecraftWorldPlugin;
use mcrs_minecraft_worldgen::bevy::{
    ProcessorListAsset, StructureAsset, StructureSetAsset, TemplateAsset, TemplatePoolAsset,
};

fn workspace_root() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn run_to_playing() -> App {
    std::env::set_current_dir(workspace_root()).unwrap();

    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(StatesPlugin);
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(mcrs_minecraft_assets::MinecraftCorePlugin);
    app.add_plugins(MinecraftWorldPlugin);
    app.finish();
    app.cleanup();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        app.update();
        if *app.world().resource::<State<AppState>>().get() == AppState::Playing {
            return app;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "never reached Playing"
        );
    }
}

#[test]
fn the_structure_registries_land_before_playing() {
    let app = run_to_playing();
    let world = app.world();

    assert_eq!(world.resource::<Assets<StructureSetAsset>>().len(), 21);
    assert_eq!(world.resource::<Assets<StructureAsset>>().len(), 52);
    assert_eq!(world.resource::<Assets<TemplatePoolAsset>>().len(), 245);
    assert_eq!(world.resource::<Assets<ProcessorListAsset>>().len(), 36);
    assert_eq!(world.resource::<Assets<TemplateAsset>>().len(), 1511);

    let only_named_beside_a_missing_sibling = world
        .resource::<AssetServer>()
        .get_handle::<TemplateAsset>(
            "minecraft/structure/ancient_city/walls/intact_horizontal_wall_bridge.nbt",
        )
        .expect("every shipped template was requested");
    assert!(
        world
            .resource::<Assets<TemplateAsset>>()
            .get(&only_named_beside_a_missing_sibling)
            .is_some()
    );

    let no_corners = world
        .resource::<AssetServer>()
        .get_handle::<TemplatePoolAsset>(
            "minecraft/worldgen/template_pool/ancient_city/walls/no_corners.json",
        )
        .expect("the pool was requested");
    let pool = world
        .resource::<Assets<TemplatePoolAsset>>()
        .get(&no_corners)
        .expect("a pool naming a template that does not ship still lands");
    let missing = mcrs_minecraft_core::ResourceLocation::minecraft(
        "ancient_city/walls/intact_horizontal_wall_stairs_5",
    );
    let handle = pool
        .deps
        .templates
        .get(&missing)
        .expect("the pool names it");
    assert!(
        world
            .resource::<Assets<TemplateAsset>>()
            .get(handle)
            .is_none()
    );
}
