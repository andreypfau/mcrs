use bevy_app::{App, FixedUpdate, Plugin, Update};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_time::{Fixed, Time, Virtual};
use mcrs_minecraft_dimension::keys::dimension::OVERWORLD;
use mcrs_minecraft_level::world::dimension::Dimension;
use mcrs_minecraft_level::world::sub_app::{AddDimensionPlugins, DimAppLabel};

use crate::host_app;

#[derive(Resource, Default)]
struct Ticks<const ALL: bool>(u32);

#[derive(Message)]
struct Ticked;

#[derive(Clone)]
struct CountTicks<const ALL: bool>;

impl<const ALL: bool> Plugin for CountTicks<ALL> {
    fn name(&self) -> &str {
        if ALL {
            "count every tick"
        } else {
            "count overworld ticks"
        }
    }

    fn build(&self, app: &mut App) {
        app.init_resource::<Ticks<ALL>>().add_systems(
            FixedUpdate,
            |mut ticks: ResMut<Ticks<ALL>>, mut ticked: MessageWriter<Ticked>| {
                ticks.0 += 1;
                if ALL {
                    ticked.write(Ticked);
                }
            },
        );
        app.add_message::<Ticked>();
    }
}

#[derive(Resource)]
struct RateSet(u32);

#[derive(Clone)]
struct Rate(u32);

impl Plugin for Rate {
    fn build(&self, app: &mut App) {
        app.insert_resource(RateSet(self.0));
    }
}

#[derive(Resource, Default)]
struct DimensionsSeen(u32);

#[derive(Clone)]
struct SeesSpawn;

impl Plugin for SeesSpawn {
    fn build(&self, app: &mut App) {
        app.init_resource::<DimensionsSeen>().add_observer(
            |_: On<Add, Dimension>, mut seen: ResMut<DimensionsSeen>| {
                seen.0 += 1;
            },
        );
    }
}

#[derive(Clone)]
struct AddsCountTicks;

impl Plugin for AddsCountTicks {
    fn build(&self, app: &mut App) {
        app.add_plugins(CountTicks::<false>);
    }
}

#[derive(Resource, Default)]
struct TimeMismatch {
    fixed: bool,
    virtual_time: bool,
}

#[derive(Clone)]
struct ReadsTime;

impl Plugin for ReadsTime {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeMismatch>()
            .add_systems(
                FixedUpdate,
                |time: Res<Time>, fixed: Res<Time<Fixed>>, mut mismatch: ResMut<TimeMismatch>| {
                    mismatch.fixed |= time.elapsed() != fixed.elapsed();
                },
            )
            .add_systems(
                Update,
                |time: Res<Time>,
                 virtual_time: Res<Time<Virtual>>,
                 mut mismatch: ResMut<TimeMismatch>| {
                    mismatch.virtual_time |= time.elapsed() != virtual_time.elapsed();
                },
            );
    }
}

fn world<'a>(app: &'a App, dimension: &str) -> &'a World {
    app.sub_app(DimAppLabel(host_app::label_of(app, dimension)))
        .world()
}

fn ticks<const ALL: bool>(app: &App, dimension: &str) -> Option<u32> {
    world(app, dimension)
        .get_resource::<Ticks<ALL>>()
        .map(|ticks| ticks.0)
}

#[test]
fn dimension_plugins_run_in_every_world_or_in_the_world_of_their_key() {
    let mut app = host_app::make_host_app();
    app.add_dimension_plugin(CountTicks::<true>)
        .add_dimension_plugin(Rate(1))
        .add_dimension_plugin(ReadsTime)
        .add_dimension_plugin_to(OVERWORLD, SeesSpawn)
        .add_dimension_plugin_to(OVERWORLD, AddsCountTicks)
        .add_dimension_plugin_to(OVERWORLD, CountTicks::<false>)
        .add_dimension_plugin_to(OVERWORLD, Rate(2));
    host_app::materialise_sub_apps(
        &mut app,
        &[
            ("minecraft:overworld", "minecraft:overworld"),
            ("minecraft:the_nether", "minecraft:the_nether"),
        ],
    );
    for _ in 0..5 {
        app.update();
    }

    assert!(ticks::<true>(&app, "minecraft:overworld").is_some_and(|n| n > 0));
    assert!(ticks::<true>(&app, "minecraft:the_nether").is_some_and(|n| n > 0));
    assert!(ticks::<false>(&app, "minecraft:overworld").is_some_and(|n| n > 0));
    assert_eq!(ticks::<false>(&app, "minecraft:the_nether"), None);
    assert_eq!(
        world(&app, "minecraft:overworld").resource::<RateSet>().0,
        2
    );
    assert_eq!(
        world(&app, "minecraft:the_nether").resource::<RateSet>().0,
        1
    );
    assert_eq!(
        world(&app, "minecraft:overworld")
            .resource::<DimensionsSeen>()
            .0,
        1
    );
    for dimension in ["minecraft:overworld", "minecraft:the_nether"] {
        let held = world(&app, dimension).resource::<Messages<Ticked>>().len();
        assert!(held <= 2, "{dimension} holds {held} messages of past ticks");
        let mismatch = world(&app, dimension).resource::<TimeMismatch>();
        assert!(
            !mismatch.fixed,
            "{dimension} reads a time other than the fixed time in FixedUpdate"
        );
        assert!(
            !mismatch.virtual_time,
            "{dimension} reads a time other than the virtual time in Update"
        );
    }
}
