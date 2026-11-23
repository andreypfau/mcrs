use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::LogLevel;
use bevy_ecs::schedule::MultiThreadedExecutor;
use bevy_ecs::schedule::ScheduleBuildSettings;
use bevy_ecs::schedule::SingleThreadedExecutor;
use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPool;
use glam::DVec3;
use mcrs_minecraft_damage::DamageSource;
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine_damage::DamagePlugin;
use mcrs_minecraft_engine_damage::DamageRequest;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Executor {
    Single,
    Multi,
}

pub(crate) fn app(executor: Executor) -> App {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let mut app = App::new();
    app.configure_sets(
        FixedUpdate,
        (
            TickSet::DomainInput,
            TickSet::ClientInput,
            TickSet::Simulate,
            TickSet::WorldWrite,
            TickSet::Project,
            TickSet::Output,
        )
            .chain(),
    );
    app.add_plugins(DamagePlugin);
    app.edit_schedule(FixedUpdate, |schedule| {
        match executor {
            Executor::Single => schedule.set_executor(SingleThreadedExecutor::default()),
            Executor::Multi => schedule.set_executor(MultiThreadedExecutor::default()),
        };
        schedule.set_build_settings(ScheduleBuildSettings {
            ambiguity_detection: LogLevel::Error,
            ..ScheduleBuildSettings::default()
        });
    });
    app
}

pub(crate) fn living(app: &mut App, uuid: u128, health: f32) -> Entity {
    let motion = MotionRow::zombie();
    let body = Body {
        position: DVec3::new(0.5, 64.0, 0.5),
        ..Body::default()
    };
    let state = EntityState::new(EntityUuid::from_u128(uuid), body, Some(Living::of(&motion)));
    app.world_mut()
        .spawn((
            MotionState(state),
            Vitals::new(health),
            HitRecord::default(),
        ))
        .id()
}

pub(crate) fn request(
    target: Entity,
    amount: f32,
    damage_type: u32,
    writer: WriterKey,
) -> DamageRequest {
    DamageRequest::new(
        target,
        Hit::new(amount, DamageSource::of(damage_type)),
        writer,
        0,
        0,
    )
}

pub(crate) fn push(app: &mut App, request: DamageRequest) {
    app.world_mut().resource_mut::<PendingHits>().push(request);
}

pub(crate) fn tick(app: &mut App) {
    app.world_mut().run_schedule(FixedUpdate);
}

pub(crate) fn uuid(app: &App, entity: Entity) -> EntityUuid {
    app.world()
        .get::<MotionState>(entity)
        .expect("a living entity")
        .0
        .key
}

pub(crate) fn damaged(app: &App) -> Vec<Damaged> {
    app.world()
        .resource::<Messages<Damaged>>()
        .iter_current_update_messages()
        .copied()
        .collect()
}
