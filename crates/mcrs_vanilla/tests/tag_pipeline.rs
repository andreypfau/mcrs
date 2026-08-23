use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_core::tag::{TagLoader, TagRegistry};
use mcrs_core::{AppState, StaticRegistry};
use mcrs_vanilla::block::{Block, tags as block_tags};
use mcrs_vanilla::MinecraftCorePlugin;

/// The vanilla registries read some files through paths relative to the
/// working directory, so the whole test runs from the workspace root.
fn workspace_root() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

#[test]
fn tags_load_resolve_and_freeze_on_the_way_to_playing() {
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
    app.add_plugins(mcrs_core::MinecraftEnginePlugin);
    app.add_plugins(MinecraftCorePlugin);
    app.finish();
    app.cleanup();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        app.update();
        let state = app.world().resource::<State<AppState>>().get().clone();
        if state == AppState::Playing {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "stuck in {state:?} before reaching Playing"
        );
    }

    assert!(
        app.world().get_resource::<TagLoader<Block>>().is_none(),
        "the loader must be consumed by the freeze"
    );

    let tags = app.world().resource::<TagRegistry<Block>>();
    let blocks = app.world().resource::<StaticRegistry<Block>>();
    let stone = blocks.id_of("minecraft:stone").expect("stone is registered");
    let dirt = blocks.id_of("minecraft:dirt").expect("dirt is registered");

    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, stone));
    assert!(!tags.contains(&block_tags::MINEABLE_PICKAXE, dirt));

    // `#minecraft:planks` is reached only through nested `#tag` entries of
    // `#minecraft:mineable/axe`.
    let oak = blocks
        .id_of("minecraft:oak_planks")
        .expect("oak_planks is registered");
    assert!(tags.contains(&block_tags::MINEABLE_AXE, oak));
}
