use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetApp, AssetPlugin, AssetServer, Assets, Handle, LoadState};
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_protocol::item::Enchantments;
use mcrs_minecraft_server::world::loot::condition::LootCondition;
use mcrs_minecraft_server::world::loot::context::BlockBreakContext;
use mcrs_minecraft_server::world::loot::{
    LootTableAsset, LootTableLoader, LootTableLoaderSettings,
};
use mcrs_minecraft_world::registries::test_registries;

fn loader_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin::default());
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.init_asset::<LootTableAsset>()
        .register_asset_loader(LootTableLoader::new(test_registries().clone()));
    app
}

fn request(app: &App, block: &str) -> Handle<LootTableAsset> {
    let settings = LootTableLoaderSettings {
        loot: Some(0),
        table_id: Some(format!("minecraft:blocks/{block}")),
    };
    app.world()
        .resource::<AssetServer>()
        .load_builder()
        .with_settings(move |s: &mut LootTableLoaderSettings| *s = settings.clone())
        .load(format!("minecraft/loot_table/blocks/{block}.json"))
}

/// Runs the app until every handle is loaded or failed, and names the failures.
fn settle(app: &mut App, handles: &[(String, Handle<LootTableAsset>)]) -> Vec<String> {
    for _ in 0..20_000 {
        app.update();
        let server = app.world().resource::<AssetServer>();
        let states: Vec<_> = handles
            .iter()
            .map(|(_, h)| server.load_state(h.id()))
            .collect();
        if states
            .iter()
            .all(|s| matches!(s, LoadState::Loaded | LoadState::Failed(_)))
        {
            return handles
                .iter()
                .zip(states)
                .filter_map(|((name, _), state)| match state {
                    LoadState::Failed(e) => Some(format!("{name}: {e}")),
                    _ => None,
                })
                .collect();
        }
        std::thread::yield_now();
    }
    panic!("loot tables did not settle");
}

fn silk_touch() -> Enchantments {
    Enchantments(vec![(
        ResourceKey::from_location(ResourceLocation::minecraft("silk_touch")),
        1,
    )])
}

/// Conditions come from the 26.4 `condition` field, written in place or named
/// from the predicate registry: only the lower half of a door drops it, and
/// glass needs silk touch, which `minecraft:tool/can_silk_touch` names.
#[test]
fn block_loot_follows_its_conditions() {
    let mut app = loader_app();
    let handles: Vec<_> = ["oak_door", "glass"]
        .map(|block| (block.to_owned(), request(&app, block)))
        .into();
    assert_eq!(settle(&mut app, &handles), Vec::<String>::new());

    let blocks = &crate::support::standalone_corpus().0;
    let door = blocks.block("minecraft:oak_door").unwrap();
    let half = |half| door.with_text(door.default_state_id, "half", half).unwrap();
    let glass = blocks.default_state("minecraft:glass");
    let silk = silk_touch();
    let tags = test_registries().tags::<Block>().unwrap();

    let cases = [
        (0, half("lower"), None, vec!["minecraft:oak_door"]),
        (0, half("upper"), None, vec![]),
        (1, glass, None, vec![]),
        (1, glass, Some(&silk), vec!["minecraft:glass"]),
    ];
    let assets = app.world().resource::<Assets<LootTableAsset>>();
    for (table, state, tool_enchantments, expected) in cases {
        let ctx = BlockBreakContext {
            blocks,
            state,
            tags: &tags,
            tool_enchantments,
        };
        let drops: Vec<_> = assets
            .get(&handles[table].1)
            .unwrap()
            .table
            .evaluate(&ctx)
            .into_iter()
            .map(|drop| (drop.item_name.to_string(), drop.count))
            .collect();
        let expected: Vec<_> = expected.into_iter().map(|i| (i.to_owned(), 1)).collect();
        assert_eq!(drops, expected, "{} state {state:?}", handles[table].0);
    }
}

#[test]
fn a_match_block_condition_tests_a_block_tag_by_membership() {
    let registries = test_registries();
    let tags = registries.tags::<Block>().unwrap();
    let blocks = &crate::support::standalone_corpus().0;
    let condition: LootCondition = registries.scope(|| {
        serde_json::from_str(
            r##"{"type":"minecraft:match_block","blocks":"#minecraft:mineable/pickaxe"}"##,
        )
        .unwrap()
    });
    let holds = |block: &str| {
        condition.check(&BlockBreakContext {
            blocks,
            state: blocks.default_state(block),
            tags: &tags,
            tool_enchantments: None,
        })
    };
    assert!(holds("minecraft:stone"));
    assert!(!holds("minecraft:dirt"));
}

mod exhaustive {
    use super::*;

    #[test]
    fn every_block_loot_table_loads() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/loot_table/blocks"
        );
        let mut app = loader_app();
        let handles: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let block = path.file_stem().unwrap().to_str().unwrap().to_owned();
                let handle = request(&app, &block);
                (block, handle)
            })
            .collect();
        assert!(handles.len() > 1000, "only {} tables", handles.len());
        assert_eq!(settle(&mut app, &handles), Vec::<String>::new());
    }
}
