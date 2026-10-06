use bevy_app::App;
use mcrs_minecraft_assets::packs::PACKS_ROOT;
use mcrs_minecraft_core::TagKey;
use mcrs_minecraft_core::resource_location::ResourceLocation;
use mcrs_minecraft_dimension_environment::dimension_type::{DimensionType, NetworkDimensionType};
use mcrs_minecraft_dimension_environment::environment::DimensionEnvironments;
use mcrs_minecraft_environment::world_clock::{ClockTimeMarkers, WorldClocks};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_registry::{Id, RegistrySet};

const OVERWORLD_CLOCK: &str = "minecraft:overworld";

fn overworld_clock(app: &App) -> Id<keys::WorldClock> {
    app.world()
        .resource::<RegistrySet>()
        .registry::<keys::WorldClock>()
        .expect("the world clock registry is loaded")
        .require_by_name(OVERWORLD_CLOCK)
        .expect("the overworld clock is registered")
}

fn members(app: &App, tag: &str) -> Vec<String> {
    let set = app.world().resource::<RegistrySet>();
    let timelines = set
        .registry::<keys::Timeline>()
        .expect("the timeline registry is loaded");
    let tags = set
        .tags::<keys::Timeline>()
        .expect("the load builds the timeline tags");
    let key = TagKey::<keys::Timeline, _>::from_location(ResourceLocation::read(tag).unwrap());
    let mut names: Vec<String> = tags
        .members(tags.get(&key).expect("the tag is resolved"))
        .map(|id| {
            timelines
                .name(id)
                .expect("a member id maps back")
                .as_str()
                .to_owned()
        })
        .collect();
    names.sort();
    names
}

pub fn the_timeline_tags_resolve_through_universal(app: &App) {
    assert_eq!(
        members(app, "minecraft:in_overworld"),
        [
            "minecraft:day",
            "minecraft:early_game",
            "minecraft:moon",
            "minecraft:villager_schedule",
        ]
    );
    assert_eq!(
        members(app, "minecraft:in_nether"),
        ["minecraft:villager_schedule"]
    );
    assert_eq!(
        members(app, "minecraft:in_end"),
        ["minecraft:villager_schedule"]
    );
}

pub fn every_dimension_builds_its_environment_from_its_tag(app: &App) {
    let environments = app.world().resource::<DimensionEnvironments>();
    let types = app
        .world()
        .resource::<RegistrySet>()
        .registry::<keys::DimensionType>()
        .expect("the dimension types are loaded");
    let environment = |name: &str| {
        environments.get(
            types
                .by_name(name)
                .unwrap_or_else(|| panic!("{name} is not a dimension type")),
        )
    };

    for id in [
        "minecraft:overworld",
        "minecraft:overworld_caves",
        "minecraft:the_nether",
        "minecraft:the_end",
    ] {
        assert!(environment(id).is_some(), "{id} has no environment");
    }

    let overworld = environment("minecraft:overworld").unwrap();
    assert_eq!(overworld.clocks(), [overworld_clock(app)]);

    let sky_light =
        mcrs_minecraft_dimension_environment::environment::EnvironmentAttributes::index(
            "minecraft:gameplay/sky_light_level",
        )
        .unwrap();
    assert!(overworld.stack(sky_light).is_dynamic());

    // `#minecraft:in_nether` pulls in villager_schedule alone, which touches
    // no sky attribute, so the Nether sky never moves.
    let nether = environment("minecraft:the_nether").unwrap();
    assert!(!nether.stack(sky_light).is_dynamic());
}

pub fn the_shipped_time_markers_reach_the_overworld_clock(app: &App) {
    let markers = app.world().resource::<ClockTimeMarkers>();
    let overworld = overworld_clock(app);

    assert!(
        app.world()
            .resource::<WorldClocks>()
            .get(overworld)
            .is_some(),
        "the clocks must be seeded before the markers are folded"
    );

    let noon = markers.get(overworld, "minecraft:noon").unwrap();
    assert_eq!((noon.ticks, noon.period_ticks), (6000, Some(24000)));
    assert!(noon.show_in_commands);
}

pub fn the_dimension_timelines_tag_round_trips_to_the_string_the_asset_holds(app: &App) {
    let set = app.world().resource::<RegistrySet>();
    let table = set
        .table("minecraft:dimension_type")
        .expect("the dimension type table is loaded");
    let dimension_types = set
        .column::<DimensionType>("minecraft:dimension_type")
        .expect("the loader parses the dimension types");

    let mut seen = 0;
    for (index, (rl, dimension_type)) in table.names().iter().zip(dimension_types).enumerate() {
        let pack = set
            .pack_of("minecraft:dimension_type", index)
            .unwrap_or_else(|| panic!("{rl} has no pack"));
        let root = match pack {
            "vanilla" => "assets/minecraft".to_owned(),
            pack => format!("assets/{PACKS_ROOT}/{pack}/minecraft"),
        };
        let raw: serde_json::Value = serde_json::from_slice(
            &std::fs::read(format!("{root}/dimension_type/{}.json", rl.path())).unwrap(),
        )
        .unwrap();

        let sent =
            set.scope(|| serde_json::to_value(NetworkDimensionType::from(dimension_type)).unwrap());
        assert_eq!(
            sent.get("timelines").and_then(|v| v.as_str()),
            raw.get("timelines").and_then(|v| v.as_str()),
            "{rl} timelines must round-trip"
        );
        seen += 1;
    }
    assert_eq!(seen, 5);
}
