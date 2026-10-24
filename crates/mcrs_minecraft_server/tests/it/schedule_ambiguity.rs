use bevy_app::App;
use bevy_ecs::schedule::{
    AmbiguousSystemConflictsWarning, ConflictingSystems, LogLevel, ScheduleBuildError,
    ScheduleBuildSettings, ScheduleBuildWarning, ScheduleGraph, Schedules, SystemKey,
};
use bevy_ecs::system::System;
use bevy_ecs::world::World;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_level::world::sub_app::DimAppLabel;
use mcrs_minecraft_server::MinecraftServerPlugins;
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};
use mcrs_minecraft_world::dimension::{DimensionEntry, Dimensions};
use mcrs_minecraft_world::worldgen::chunk_generator::ChunkGenerator;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets");

fn is_bevy_state_system(graph: &ScheduleGraph, key: SystemKey) -> bool {
    graph.systems.get(key).is_some_and(|system| {
        let name = system.name().to_string();
        let parts = name
            .strip_prefix("Pipe(")
            .and_then(|piped| piped.strip_suffix(')'))
            .map_or_else(|| vec![name.as_str()], |piped| piped.split(", ").collect());
        parts.iter().all(|part| part.starts_with("bevy_state::"))
    })
}

// bevy_state's exclusive transition runners are unordered against its own entity-scoping systems.
fn without_bevy_state_pairs(
    error: ScheduleBuildError,
    graph: &ScheduleGraph,
) -> Option<ScheduleBuildError> {
    let ScheduleBuildError::Elevated(ScheduleBuildWarning::Ambiguity(
        AmbiguousSystemConflictsWarning(ConflictingSystems(pairs)),
    )) = error
    else {
        return Some(error);
    };
    let ours: Vec<_> = pairs
        .into_iter()
        .filter(|(a, b, _)| !(is_bevy_state_system(graph, *a) && is_bevy_state_system(graph, *b)))
        .collect();
    (!ours.is_empty()).then_some(ScheduleBuildError::Elevated(
        ScheduleBuildWarning::Ambiguity(AmbiguousSystemConflictsWarning(ConflictingSystems(ours))),
    ))
}

fn ambiguous_schedules(app: &str, world: &mut World) -> Vec<String> {
    let labels: Vec<_> = world
        .resource::<Schedules>()
        .iter()
        .map(|(_, schedule)| schedule.label())
        .collect();
    labels
        .into_iter()
        .filter_map(|label| {
            world
                .try_schedule_scope(label, |world, schedule| {
                    if !schedule.is_changed() && schedule.systems_len() > 0 {
                        return Some(format!(
                            "{app} {label:?}: built before its ambiguity check, so it is not checked"
                        ));
                    }
                    let settings = schedule.get_build_settings();
                    schedule.set_build_settings(ScheduleBuildSettings {
                        ambiguity_detection: LogLevel::Error,
                        ..settings.clone()
                    });
                    let error = schedule.initialize(world).err()?;
                    let report = without_bevy_state_pairs(error, schedule.graph()).map(|error| {
                        format!("{app} {label:?}: {}", error.to_string(schedule.graph(), world))
                    });
                    // Rebuilt leniently so the app still runs far enough to check the dimension.
                    schedule.set_build_settings(settings);
                    schedule.initialize(world).ok();
                    report
                })
                .ok()
                .flatten()
        })
        .collect()
}

#[test]
fn no_simulation_schedule_has_an_unordered_conflicting_pair() {
    let mut app = App::new();
    let mut plugins = MinecraftServerPlugins::embedded();
    plugins.core.asset_path = Some(ASSETS.to_owned());
    plugins.network.announce_on_lan = false;
    plugins.worlds.dimensions = Some(Dimensions::from([(
        ResourceKey::from_location("mcrs:ambiguity".parse().expect("a valid identifier")),
        DimensionEntry {
            dimension_type: crate::support::dimension_type("minecraft:overworld"),
            generator: ChunkGenerator::Debug,
        },
    )]));
    app.add_plugins(plugins);
    app.finish();
    app.cleanup();
    let mut reports = ambiguous_schedules("server", app.world_mut());

    crate::support::update_until_playing(&mut app);
    drain_dim_spawn_queue(&mut app);
    let dimension = app
        .world_mut()
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<DimSubAppHandle>>()
        .single(app.world())
        .expect("one dimension is spawned");
    let dimension_world = app
        .get_sub_app_mut(DimAppLabel(dimension))
        .expect("the dimension has a sub-app")
        .world_mut();
    reports.extend(ambiguous_schedules("dimension", dimension_world));

    assert!(reports.is_empty(), "{}", reports.join("\n"));
}
