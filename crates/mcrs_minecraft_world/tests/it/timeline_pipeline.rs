use bevy_app::App;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_datapack::PACKS_ROOT;
use mcrs_minecraft_dimension::{Dimension, DimensionType};
use mcrs_minecraft_dimension_environment::dimension_type::DimensionTypeFile;
use mcrs_minecraft_dimension_environment::environment::EnvironmentAttributes;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::WorldClock;
use mcrs_minecraft_registries::{CurrentRegistries, Resolve, Resolver};

use crate::common::tag_members;

const OVERWORLD_CLOCK: &str = "minecraft:overworld";

fn overworld_clock(app: &App) -> Id<WorldClock> {
    app.world()
        .resource::<CurrentRegistries>()
        .id(&ResourceKey::from_location(
            OVERWORLD_CLOCK.parse().unwrap(),
        ))
        .expect("the overworld clock is registered")
}

fn members(app: &App, tag: &str) -> Vec<String> {
    let mut names = tag_members::<Timeline>(app.world().resource::<CurrentRegistries>(), tag);
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
    let set = app.world().resource::<CurrentRegistries>();
    let environment = |name: &str| {
        let dimension = ResourceKey::<Dimension>::from_location(name.parse().unwrap());
        EnvironmentAttributes::of_dimension(
            set,
            &dimension,
            set.id::<DimensionType>(&ResourceKey::from_location(name.parse().unwrap()))
                .unwrap_or_else(|error| panic!("{name}: {error}")),
        )
        .ok()
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

pub fn the_dimension_timelines_tag_round_trips_to_the_string_the_asset_holds(app: &App) {
    let set = app.world().resource::<CurrentRegistries>();
    let registry = set
        .registry::<DimensionTypeFile>()
        .expect("the dimension types are loaded");

    let mut seen = 0;
    for (id, rl, dimension_type) in registry.iter() {
        let pack = set
            .origin(id)
            .unwrap_or_else(|| panic!("{rl} has no pack"))
            .id();
        let root = match pack {
            "vanilla" => mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
            pack => std::path::PathBuf::from(format!(
                "assets/{PACKS_ROOT}/{}/minecraft",
                pack.trim_start_matches("file/")
            )),
        };
        let raw: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join(format!("dimension_type/{}.json", rl.path()))).unwrap(),
        )
        .unwrap();

        let synced = dimension_type.clone().synced().name(&**set).unwrap();
        let sent = serde_json::to_value(synced).unwrap();
        assert_eq!(
            sent.get("timelines").and_then(|v| v.as_str()),
            raw.get("timelines").and_then(|v| v.as_str()),
            "{rl} timelines must round-trip"
        );
        seen += 1;
    }
    assert_eq!(seen, 5);
}
