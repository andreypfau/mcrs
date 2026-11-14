use bevy_app::FixedUpdate;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use mcrs_minecraft_falling_block::Settings;
use mcrs_minecraft_vanilla_falling_block::FallingBlockSet;
use mcrs_minecraft_vanilla_falling_block::Landed;
use mcrs_minecraft_vanilla_falling_block::PendingFall;
use mcrs_voxel::BlockPos;

use crate::fixture::app;
use crate::fixture::block;
use crate::fixture::drop_block;
use crate::fixture::falling_blocks;
use crate::fixture::item_kinds;
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
    assert!(falling_blocks(world).is_empty());
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
    assert!(item_kinds(world).is_empty());
}
