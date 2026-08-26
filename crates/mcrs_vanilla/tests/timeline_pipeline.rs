use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_asset::{AssetServer, Assets};
use bevy_state::app::StatesPlugin;
use bevy_state::state::State;
use mcrs_core::AppState;
use mcrs_core::registry::snapshot::rl_from_asset_path;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_core::tag::{DynRegistryIndex, DynTagRegistry, TagKey};
use mcrs_vanilla::MinecraftWorldPlugin;
use mcrs_vanilla::dimension::dimension_type::{DimensionType, NetworkDimensionType};
use mcrs_vanilla::environment::DimensionEnvironments;
use mcrs_vanilla::timeline::Timeline;
use mcrs_vanilla::world_clock::{ClockTimeMarkers, WorldClocks};

const OVERWORLD_CLOCK: &str = "minecraft:overworld";

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
    app.add_plugins(mcrs_core::MinecraftCorePlugin);
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
    let tags = app.world().resource::<DynTagRegistry<Timeline>>();
    let index = app.world().resource::<DynRegistryIndex<Timeline>>();
    let key = TagKey::<Timeline, _>::from_location(ResourceLocation::parse(tag).unwrap());
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
fn the_timeline_tags_resolve_through_universal() {
    let app = run_to_playing();

    assert_eq!(
        members(&app, "minecraft:in_overworld"),
        [
            "minecraft:day",
            "minecraft:early_game",
            "minecraft:moon",
            "minecraft:villager_schedule",
        ]
    );
    assert_eq!(
        members(&app, "minecraft:in_nether"),
        ["minecraft:villager_schedule"]
    );
    assert_eq!(
        members(&app, "minecraft:in_end"),
        ["minecraft:villager_schedule"]
    );
}

#[test]
fn every_dimension_builds_its_environment_from_its_tag() {
    let app = run_to_playing();
    let environments = app.world().resource::<DimensionEnvironments>();

    for id in [
        "minecraft:overworld",
        "minecraft:overworld_caves",
        "minecraft:the_nether",
        "minecraft:the_end",
    ] {
        assert!(environments.get(id).is_some(), "{id} has no environment");
    }

    let overworld = environments.get("minecraft:overworld").unwrap();
    assert_eq!(
        overworld
            .clocks()
            .iter()
            .map(|c| c.as_str())
            .collect::<Vec<_>>(),
        [OVERWORLD_CLOCK]
    );

    let sky_light = mcrs_vanilla::environment::EnvironmentAttributes::index(
        "minecraft:gameplay/sky_light_level",
    )
    .unwrap();
    assert!(overworld.stack(sky_light).is_dynamic());

    // `#minecraft:in_nether` pulls in villager_schedule alone, which touches
    // no sky attribute, so the Nether sky never moves.
    let nether = environments.get("minecraft:the_nether").unwrap();
    assert!(!nether.stack(sky_light).is_dynamic());
}

#[test]
fn the_shipped_time_markers_reach_the_overworld_clock() {
    let app = run_to_playing();
    let markers = app.world().resource::<ClockTimeMarkers>();

    assert!(
        app.world()
            .resource::<WorldClocks>()
            .get(OVERWORLD_CLOCK)
            .is_some(),
        "the clocks must be seeded before the markers are folded"
    );

    let names: Vec<&str> = markers
        .of_clock(OVERWORLD_CLOCK)
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "minecraft:day",
            "minecraft:midnight",
            "minecraft:night",
            "minecraft:noon",
            "minecraft:roll_village_siege",
            "minecraft:wake_up_from_sleep",
        ]
    );

    let noon = markers.get(OVERWORLD_CLOCK, "minecraft:noon").unwrap();
    assert_eq!((noon.ticks, noon.period_ticks), (6000, Some(24000)));
    assert!(noon.show_in_commands);

    assert_eq!(markers.of_clock("minecraft:the_end").count(), 0);
    assert!(markers.get("minecraft:the_end", "minecraft:noon").is_none());
}

#[test]
fn the_dimension_timelines_tag_round_trips_to_the_string_the_asset_holds() {
    let app = run_to_playing();
    let asset_server = app.world().resource::<AssetServer>();

    let mut seen = 0;
    for (id, dimension_type) in app.world().resource::<Assets<DimensionType>>().iter() {
        let Some(rl) = asset_server
            .get_path(id)
            .and_then(|path| rl_from_asset_path(path.path()))
        else {
            continue;
        };
        let raw: serde_json::Value = serde_json::from_slice(
            &std::fs::read(format!(
                "assets/minecraft/dimension_type/{}.json",
                rl.path()
            ))
            .unwrap(),
        )
        .unwrap();

        let sent = NetworkDimensionType::from(dimension_type);
        assert_eq!(
            sent.timelines.as_deref(),
            raw.get("timelines").and_then(|v| v.as_str()),
            "{rl} timelines must round-trip"
        );
        seen += 1;
    }
    assert_eq!(seen, 4);
}
