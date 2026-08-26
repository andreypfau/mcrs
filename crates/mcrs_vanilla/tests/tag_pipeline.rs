use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_minecraft_core::AppState;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_core::tag::TagLoader;
use mcrs_minecraft_core::tag::key::TagKey;
use mcrs_minecraft_core::tag::registry::DynTagRegistry;
use mcrs_vanilla::MinecraftWorldPlugin;
use mcrs_vanilla::block::definition::Blocks;
use mcrs_vanilla::block::{Block, tags as block_tags};

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
    app.add_plugins(mcrs_minecraft_core::MinecraftCorePlugin);
    app.add_plugins(MinecraftWorldPlugin);
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
        app.world()
            .get_resource::<TagLoader<Block, u32>>()
            .is_none(),
        "the loader must be consumed by the freeze"
    );

    let tags = app.world().resource::<DynTagRegistry<Block>>();
    let blocks = app.world().resource::<Blocks>();
    let index = |name: &str| blocks.index_of(name).expect("the corpus declares it");

    assert!(tags.contains(&block_tags::MINEABLE_PICKAXE, index("minecraft:stone")));
    assert!(!tags.contains(&block_tags::MINEABLE_PICKAXE, index("minecraft:dirt")));

    // `#minecraft:planks` is reached only through nested `#tag` entries of
    // `#minecraft:mineable/axe`.
    assert!(tags.contains(&block_tags::MINEABLE_AXE, index("minecraft:oak_planks")));

    // A block no static registry ever named still lands in its tags.
    assert!(tags.contains(
        &block_tags::MINEABLE_PICKAXE,
        index("minecraft:polished_tuff_stairs")
    ));
    assert!(tags.contains(
        &block_tags::MINEABLE_AXE,
        index("minecraft:mangrove_trapdoor")
    ));
    assert!(tags.contains(&block_tags::WOOL, index("minecraft:magenta_wool")));

    // And a tag no Rust constant names is there, because the pack ships it.
    let stairs =
        TagKey::<Block, _>::from_location(ResourceLocation::parse("minecraft:stairs").unwrap());
    assert!(tags.contains(&stairs, index("minecraft:polished_tuff_stairs")));
    assert!(!tags.contains(&stairs, index("minecraft:stone")));

    let resolved = tags.iter().count();
    println!("block tags resolved: {resolved}");
    assert!(
        resolved > block_tags::ALL_BLOCK_TAGS.len(),
        "the loader must pick up more than the tags Rust names: {resolved}"
    );
}
