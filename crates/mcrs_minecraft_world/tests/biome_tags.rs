use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_assets::snapshot::RegistrySnapshot;
use mcrs_minecraft_assets::tag::DynTagRegistry;
use mcrs_minecraft_core::TagKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_registry::DynRegistryIndex;
use mcrs_minecraft_world::MinecraftWorldPlugin;
use mcrs_minecraft_world::biome::Biome;

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

fn members(app: &App, tag: &str) -> Vec<String> {
    let tags = app.world().resource::<DynTagRegistry<Biome>>();
    let index = app.world().resource::<DynRegistryIndex<Biome>>();
    let key = TagKey::<Biome, _>::from_location(ResourceLocation::parse(tag).unwrap());
    let mut names: Vec<String> = tags
        .get(&key)
        .expect("the tag is resolved")
        .iter()
        .map(|id| {
            index
                .location(id)
                .expect("a member id maps back")
                .as_str()
                .to_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn the_shipped_biome_tags_resolve() {
    let app = run_to_playing();

    assert_eq!(
        members(&app, "minecraft:has_structure/village_plains"),
        ["minecraft:meadow", "minecraft:plains"]
    );

    let biased = members(&app, "minecraft:stronghold_biased_to");
    assert_eq!(biased.len(), 38);
    assert!(biased.contains(&"minecraft:plains".to_owned()));

    let nested = members(&app, "minecraft:has_structure/stronghold");
    assert_eq!(nested, members(&app, "minecraft:is_overworld"));
    assert_eq!(nested.len(), 56);
}

#[test]
fn the_biome_index_and_snapshot_agree_on_the_id_space() {
    let app = run_to_playing();
    let index = app.world().resource::<DynRegistryIndex<Biome>>();
    let snapshot = app.world().resource::<RegistrySnapshot<Biome>>();

    assert_eq!(index.len(), snapshot.len());
    assert!(!index.is_empty());
    for (id, entry) in snapshot.iter() {
        assert_eq!(
            index.location(id).map(|l| l.as_str()),
            Some(entry.location.as_str())
        );
    }
}
