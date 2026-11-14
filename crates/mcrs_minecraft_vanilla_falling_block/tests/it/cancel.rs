use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_falling_block::AtLanding;
use mcrs_minecraft_falling_block::Fall;
use mcrs_minecraft_falling_block::ItemDrop;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_falling_block::Start;
use mcrs_minecraft_vanilla_falling_block::FallingBlockSet;
use mcrs_minecraft_vanilla_falling_block::Landed;
use mcrs_minecraft_vanilla_falling_block::PendingFall;
use mcrs_voxel::BlockPos;

use crate::fixture::app;
use crate::fixture::block;
use crate::fixture::drop_block;
use crate::fixture::falling_blocks;
use crate::fixture::item_kinds;
use crate::fixture::item_of;
use crate::fixture::run;
use crate::fixture::state;

const START: BlockPos = BlockPos::new(0, 5, 0);

fn cancel_starts(mut commands: Commands, pending: Query<Entity, With<PendingFall>>) {
    for entity in &pending {
        commands.entity(entity).despawn();
    }
}

fn cancel_landings(mut commands: Commands, landed: Query<Entity, With<Landed>>) {
    for entity in &landed {
        commands.entity(entity).remove::<Landed>();
    }
}

#[test]
fn a_start_that_its_review_cancels_leaves_the_block() {
    let mut app = app(Settings::default());
    app.add_systems(
        FixedUpdate,
        cancel_starts.in_set(FallingBlockSet::StartReview),
    );
    drop_block(app.world_mut(), START, state("sand"));
    run(&mut app, 5);
    let world = app.world_mut();
    assert_eq!(block(world, START), state("sand"));
    assert!(
        falling_blocks(world).is_empty(),
        "a cancelled start makes no falling block"
    );
}

#[test]
fn a_landing_that_its_review_cancels_places_nothing() {
    let mut app = app(Settings::default());
    app.add_systems(
        FixedUpdate,
        cancel_landings.in_set(FallingBlockSet::LandingReview),
    );
    drop_block(app.world_mut(), START, state("sand"));
    run(&mut app, 60);
    let world = app.world_mut();
    assert_eq!(block(world, BlockPos::new(0, 0, 0)), state("air"));
    assert_eq!(falling_blocks(world).len(), 1);
    assert!(
        item_kinds(world).is_empty(),
        "a cancelled landing drops no item"
    );
}

#[derive(Resource, Default)]
struct FirstCancelled(bool);

fn cancel_the_first_landing(
    mut commands: Commands,
    landed: Query<(Entity, &MotionState), With<Landed>>,
    mut cancelled: ResMut<FirstCancelled>,
) {
    if cancelled.0 {
        return;
    }
    let first = landed
        .iter()
        .min_by_key(|(_, MotionState(motion))| motion.key);
    if let Some((entity, _)) = first {
        commands.entity(entity).remove::<Landed>();
        cancelled.0 = true;
    }
}

#[test]
fn a_landing_at_a_cell_whose_first_landing_its_review_cancels_places_the_block() {
    let mut app = app(Settings::default());
    app.init_resource::<FirstCancelled>().add_systems(
        FixedUpdate,
        cancel_the_first_landing.in_set(FallingBlockSet::LandingReview),
    );
    let start = Start {
        leftover: state("air"),
        position: DVec3::new(0.5, 5.0, 0.5),
        falling_block: Fall {
            carried: state("sand"),
            item_drop: ItemDrop::Allowed,
            at_landing: AtLanding::Settle,
            start: *START,
        },
    };
    app.world_mut().spawn(PendingFall(start));
    app.world_mut().spawn(PendingFall(start));
    for _ in 0..60 {
        run(&mut app, 1);
        if app.world().resource::<FirstCancelled>().0 {
            break;
        }
    }
    let world = app.world_mut();
    assert!(world.resource::<FirstCancelled>().0, "both blocks land");
    assert_eq!(block(world, BlockPos::new(0, 0, 0)), state("sand"));
    assert_eq!(falling_blocks(world).len(), 1);
    assert!(
        item_kinds(world).is_empty(),
        "the second landing takes the cell that the cancelled one left free"
    );
    run(&mut app, 1);
    assert_eq!(item_kinds(app.world_mut()), vec![item_of("sand")]);
}
