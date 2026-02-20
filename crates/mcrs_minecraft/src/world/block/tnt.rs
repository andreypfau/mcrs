use crate::world::entity::explosive::primed_tnt::{
    Detonator, PrimedTntBundle, DEFAULT_FUSE_DURATION,
};
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::player_action::PlayerWillDestroyBlock;
use crate::world::entity::EntityOwner;
use crate::world::explosion::BlockExplodedEvent;
use bevy_app::Plugin;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::On;
use bevy_ecs::query::{Has, With};
use bevy_ecs::system::{Commands, Query};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::dimension::InDimension;
use mcrs_vanilla::block::minecraft::tnt::{DEFAULT_STATE, UNSTABLE_STATE};
use rand::{rng, Rng, RngExt};

pub struct TntBlockPlugin;

impl Plugin for TntBlockPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(bevy_app::FixedUpdate, player_will_destroy_tnt);
        app.add_observer(tnt_block_exploded);
    }
}

fn player_will_destroy_tnt(
    mut messages: MessageReader<PlayerWillDestroyBlock>,
    player: Query<(Has<InstantBuild>, &InDimension), With<Player>>,
    mut commands: Commands,
) {
    messages.read().for_each(|event| {
        if event.block_state != UNSTABLE_STATE.id {
            return;
        }
        let Some((instant_build, dim)) = player.get(event.player).ok() else {
            return;
        };
        if instant_build {
            return;
        }
        commands.spawn((
            PrimedTntBundle::new(
                *dim,
                Transform::from_translation(event.block_pos.as_dvec3() + DVec3::new(0.5, 0.5, 0.5)),
            ),
            EntityOwner(event.player),
            Detonator(event.player),
        ));
    });
}

fn tnt_block_exploded(event: On<BlockExplodedEvent>, mut commands: Commands) {
    let blk = event.block_state_id;
    if blk != DEFAULT_STATE.id && blk != UNSTABLE_STATE.id {
        return;
    }
    let fuse = rng().random_range(0..(DEFAULT_FUSE_DURATION / 4)) + DEFAULT_FUSE_DURATION / 8;

    commands.spawn(
        PrimedTntBundle::new(
            InDimension(event.dimension),
            Transform::from_translation(event.block_pos.as_dvec3() + DVec3::new(0.5, 0.0, 0.5)),
        )
        .with_fuse(fuse),
    );
}
