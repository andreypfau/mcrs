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
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::MaxDamage;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct Tick;

fn read_max_damage(_: Query<Effective<MaxDamage>>) {}

fn write_damage(_: Query<&mut Damage>) {}

fn write_max_damage(_: Query<&mut MaxDamage>) {}

#[test]
fn effective_beside_an_unordered_writer_is_ambiguous_only_on_its_type() {
    let cases = [
        (schedule((read_max_damage, write_damage)), true),
        (schedule((read_max_damage, write_max_damage)), false),
    ];
    for (mut schedule, builds) in cases {
        let mut world = World::new();
        assert_eq!(schedule.initialize(&mut world).is_ok(), builds);
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
