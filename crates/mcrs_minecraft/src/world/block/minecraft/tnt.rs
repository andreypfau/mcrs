use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::entity::EntityOwner;
use crate::world::entity::explosive::primed_tnt::{
    DEFAULT_FUSE_DURATION, Detonator, PrimedTntBundle,
};
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::player_action::PlayerWillDestroyBlock;
use crate::world::explosion::BlockExplodedEvent;
use crate::world::material::map::MapColor;
use bevy_app::Plugin;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::On;
use bevy_ecs::query::{Has, With};
use bevy_ecs::system::{Commands, Query};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::BlockStateId;
use rand::{Rng, rng};

pub const BLOCK: Block = Block {
    identifier: mcrs_protocol::ident!("tnt"),
    properties: &PROPERTIES,
    default_state: &DEFAULT_STATE,
    states: &[UNSTABLE_STATE, DEFAULT_STATE],
};

pub const UNSTABLE_STATE: BlockState = BlockState {
    id: BlockStateId(2140),
};

pub const DEFAULT_STATE: BlockState = BlockState {
    id: BlockStateId(2141),
};

pub const PROPERTIES: Properties = Properties::new()
    .with_map_color(MapColor::FIRE)
    .with_strength(0.0)
    .ignited_by_lava()
    .instant_break();

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
    // random int from 0 to DEFAULT_FUSE_DURATION
    let fuse = rng().random_range(0..(DEFAULT_FUSE_DURATION / 4)) + DEFAULT_FUSE_DURATION / 8;

    commands.spawn(
        PrimedTntBundle::new(
            InDimension(event.dimension),
            Transform::from_translation(event.block_pos.as_dvec3() + DVec3::new(0.5, 0.0, 0.5)),
        )
        .with_fuse(fuse),
    );
}
