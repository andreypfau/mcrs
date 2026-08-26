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
use bevy_ecs::system::{Commands, Query, Res};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::dimension::InDimension;
use mcrs_vanilla::block::definition::Blocks;
use mcrs_vanilla::block::definition::schema::PropertyValue;
use rand::{RngExt, rng};

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
    blocks: Res<Blocks>,
    mut commands: Commands,
) {
    messages.read().for_each(|event| {
        if !is_unstable_tnt(&blocks, event.block_state) {
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

fn is_tnt(blocks: &Blocks, state: mcrs_protocol::BlockStateId) -> bool {
    blocks.owner(state).identifier.as_str() == "minecraft:tnt"
}

fn is_unstable_tnt(blocks: &Blocks, state: mcrs_protocol::BlockStateId) -> bool {
    is_tnt(blocks, state)
        && blocks.owner(state).value_of(state, "unstable") == Some(&PropertyValue::Bool(true))
}

fn tnt_block_exploded(
    event: On<BlockExplodedEvent>,
    blocks: Res<Blocks>,
    mut commands: Commands,
) {
    if !is_tnt(&blocks, event.block_state_id) {
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
