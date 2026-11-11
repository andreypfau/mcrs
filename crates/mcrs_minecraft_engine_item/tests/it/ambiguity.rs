use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::MultiThreadedExecutor;
use bevy_ecs::schedule::Schedule;
use bevy_ecs::schedule::ScheduleBuildSettings;
use bevy_ecs::schedule::ScheduleLabel;
use bevy_ecs::system::Query;
use bevy_ecs::system::ScheduleSystem;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_item::Effective;
use mcrs_minecraft_engine_item::Removed;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::MaxDamage;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct Tick;

fn read_max_damage(_: Query<Effective<MaxDamage>>) {}

fn write_damage(_: Query<&mut Damage>) {}

fn write_max_damage(_: Query<&mut MaxDamage>) {}

fn write_removed(_: Query<&mut Removed>) {}

#[test]
fn effective_beside_an_unordered_writer_is_ambiguous_only_on_its_type() {
    let cases = [
        (schedule((read_max_damage, write_damage)), true),
        (schedule((read_max_damage, write_max_damage)), false),
        (schedule((read_max_damage, write_removed)), false),
    ];
    for (mut schedule, unambiguous) in cases {
        let mut world = World::new();
        assert_eq!(schedule.initialize(&mut world).is_ok(), unambiguous);
    }
}

fn schedule<M>(systems: impl IntoScheduleConfigs<ScheduleSystem, M>) -> Schedule {
    let mut schedule = Schedule::new(Tick);
    schedule
        .set_executor(MultiThreadedExecutor::new())
        .set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..ScheduleBuildSettings::default()
        })
        .add_systems(systems);
    schedule
}
