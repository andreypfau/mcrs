use std::panic::{AssertUnwindSafe, catch_unwind};

use bevy_app::{App, AppLabel};
use bevy_ecs::entity::Entity;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::DimensionType;
use mcrs_minecraft_level::session::PlayerSessionCounter;
use mcrs_minecraft_level::world::dimension::{
    DimensionTypeConfig, DimensionTypeId, HasSkyLight, HasWeather,
};
use mcrs_minecraft_level::world::sub_app::DimAppLabel;
use mcrs_minecraft_server::world::bus::InboundPlayerSpawn;
use mcrs_minecraft_server::world::enqueue_dim_spawns;
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};
use mcrs_minecraft_server::world_options::DimensionList;
use mcrs_minecraft_world::dimension::Dimensions;
use mcrs_minecraft_world::registries::test_registries;
use mcrs_minecraft_world::worldgen::world_preset::WorldPreset;

use crate::{host_app, support};
use mcrs_minecraft_dimension::Dimension;

fn host_spawning(dimensions: &Dimensions) -> App {
    let mut app = host_app::make_host_app();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
    app.add_message::<InboundPlayerSpawn>();
    app.insert_resource(DimensionList::new(support::baked(
        dimensions,
        test_registries(),
    )));
    app.world_mut()
        .run_system_once(enqueue_dim_spawns)
        .expect("the spawn requests are enqueued");
    drain_dim_spawn_queue(&mut app);
    app
}

fn label_of(app: &mut App, dimension: &str) -> Entity {
    let mut handles = app
        .world_mut()
        .query::<(Entity, &DimSubAppHandle, &ResourceKey<Dimension>)>();
    handles
        .iter(app.world())
        .find(|(_, _, key)| key.as_str() == dimension)
        .map(|(entity, _, _)| entity)
        .unwrap_or_else(|| panic!("{dimension} was spawned"))
}

fn in_world<T>(app: &mut App, label: Entity, read: impl FnOnce(&mut World) -> T) -> T {
    let world = app
        .sub_apps_mut()
        .sub_apps
        .get_mut(&DimAppLabel(label).intern())
        .expect("the dimension sub-app exists")
        .world_mut();
    read(world)
}

fn config_of(app: &mut App, label: Entity) -> DimensionTypeConfig {
    in_world(app, label, |world| {
        *world
            .query::<&DimensionTypeConfig>()
            .single(world)
            .expect("one dimension in its world")
    })
}

fn row(failed: &mut Vec<String>, name: &str, check: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(check)) {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("panicked");
        failed.push(format!("{name}: {message}"));
    }
}

fn rows_hold(failed: Vec<String>) {
    assert!(
        failed.is_empty(),
        "failing rows, the first is the cause:\n{}",
        failed.join("\n")
    );
}

fn a_dimension_named_unlike_its_type_spawns_with_its_entry_type(app: &mut App) {
    let overworld = label_of(app, "minecraft:overworld");

    let config = config_of(app, overworld);
    assert_eq!((config.min_y, config.height), (0, 128));
    assert_eq!(config.section_count, 8);

    let login = host_app::join(app, overworld, Vec::new().into());
    assert_eq!(
        login.player_spawn_info.dimension_type_id.number(),
        support::dimension_type("minecraft:beta").number(),
        "the login names the beta type, not the type that shares the dimension's name"
    );
}

fn every_shipped_preset_dimension_spawns_with_its_entry_type(
    name: &str,
    dimensions: &Dimensions,
    app: &mut App,
) {
    let types = test_registries()
        .entries::<DimensionType, DimensionType>()
        .expect("the dimension types are loaded");
    for (key, entry) in support::baked(dimensions, test_registries()) {
        let label = label_of(app, key.as_str());
        let (spawned, has_sky) = in_world(app, label, |world| {
            let spawned = *world
                .query::<&DimensionTypeId>()
                .single(world)
                .expect("one dimension in its world");
            let has_sky = world.query::<&HasSkyLight>().iter(world).count() == 1;
            (spawned, has_sky)
        });
        assert_eq!(spawned.0, entry.dimension_type, "{name}: {key}");
        assert_eq!(
            has_sky,
            types.as_slice()[entry.dimension_type.index()].has_skylight,
            "{name}: {key}"
        );
    }
}

fn every_join_into_a_dimension_sends_the_type_of_its_entry(app: &mut App) {
    let overworld = label_of(app, "minecraft:overworld");
    host_app::enqueue_spawn(app, "test:nether", "minecraft:the_nether");
    host_app::enqueue_spawn(app, "minecraft:the_end", "minecraft:the_end");
    drain_dim_spawn_queue(app);
    let nether = label_of(app, "test:nether");
    let end = label_of(app, "minecraft:the_end");

    let numbers: Vec<u16> = [overworld, nether, end, overworld]
        .into_iter()
        .map(|label| {
            host_app::join(app, label, Vec::new().into())
                .player_spawn_info
                .dimension_type_id
                .number()
        })
        .collect();
    assert_eq!(
        numbers,
        [
            support::dimension_type("minecraft:beta").number(),
            support::dimension_type("minecraft:the_nether").number(),
            support::dimension_type("minecraft:the_end").number(),
            support::dimension_type("minecraft:beta").number(),
        ]
    );
}

#[test]
fn every_preset_spawns_its_dimensions_with_their_entry_types_and_beta_joins_send_them() {
    let presets = test_registries().registry::<WorldPreset>().unwrap();
    assert!(presets.len() > 1, "the shipped presets are loaded");

    let mut failed = Vec::new();
    let mut beta_rows_ran = false;
    for preset_id in presets.ids() {
        let name = presets.name(preset_id).unwrap().as_str();
        let dimensions = support::preset(name);
        let mut app = host_spawning(&dimensions);
        row(
            &mut failed,
            &format!("{name} spawns each dimension with its entry type"),
            || {
                every_shipped_preset_dimension_spawns_with_its_entry_type(
                    name,
                    &dimensions,
                    &mut app,
                )
            },
        );
        if name == "minecraft:beta" {
            beta_rows_ran = true;
            row(
                &mut failed,
                "a dimension named unlike its type spawns with its entry type",
                || a_dimension_named_unlike_its_type_spawns_with_its_entry_type(&mut app),
            );
            row(
                &mut failed,
                "every join into a dimension sends the type of its entry",
                || every_join_into_a_dimension_sends_the_type_of_its_entry(&mut app),
            );
        }
    }
    assert!(beta_rows_ran, "the beta preset is shipped");
    rows_hold(failed);
}

#[test]
fn dimensions_on_a_bare_host_have_the_sections_and_weather_of_their_type() {
    let bounds = [
        ("minecraft:beta", 128, 0, 8, 0),
        ("minecraft:overworld", 384, -64, 24, -4),
        ("minecraft:the_nether", 256, 0, 16, 0),
    ];
    let weather = [
        ("minecraft:overworld", "minecraft:overworld", true),
        ("minecraft:the_end", "minecraft:the_end", false),
        ("test:end_like", "minecraft:the_end", true),
        ("minecraft:the_nether", "minecraft:the_nether", false),
    ];
    let weather_of_another_type_under_a_used_key =
        [("minecraft:the_end", "minecraft:overworld", false)];

    let mut app = host_app::make_host_app();
    for (index, (dimension_type, ..)) in bounds.iter().enumerate() {
        host_app::enqueue_spawn(&mut app, &format!("test:bounds_{index}"), dimension_type);
    }
    for (key, dimension_type, _) in weather {
        host_app::enqueue_spawn(&mut app, key, dimension_type);
    }
    drain_dim_spawn_queue(&mut app);

    let mut failed = Vec::new();
    for (index, (dimension_type, height, min_y, sections, min_section)) in
        bounds.into_iter().enumerate()
    {
        row(
            &mut failed,
            &format!("{dimension_type} gives vanilla sections"),
            || {
                let label = label_of(&mut app, &format!("test:bounds_{index}"));
                let config = config_of(&mut app, label);
                assert_eq!(
                    (config.height, config.min_y, config.section_count),
                    (height, min_y, sections),
                    "{dimension_type}"
                );
                assert_eq!(config.min_y >> 4, min_section, "{dimension_type}");
            },
        );
    }
    for (key, dimension_type, expected) in weather {
        row(
            &mut failed,
            &format!("{key} of type {dimension_type} has weather"),
            || assert_has_weather(&mut app, key, dimension_type, expected),
        );
    }

    for (key, dimension_type, expected) in weather_of_another_type_under_a_used_key {
        let mut apart = host_app::make_host_app();
        host_app::enqueue_spawn(&mut apart, key, dimension_type);
        drain_dim_spawn_queue(&mut apart);
        row(
            &mut failed,
            &format!("{key} of type {dimension_type} has weather"),
            || assert_has_weather(&mut apart, key, dimension_type, expected),
        );
    }
    rows_hold(failed);
}

fn assert_has_weather(app: &mut App, key: &str, dimension_type: &str, expected: bool) {
    let label = label_of(app, key);
    let has_weather = in_world(app, label, |world| {
        world.query::<&HasWeather>().iter(world).count() == 1
    });
    assert_eq!(has_weather, expected, "{key} of type {dimension_type}");
}
