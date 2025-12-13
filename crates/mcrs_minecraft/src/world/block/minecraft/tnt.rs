use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::entity::EntityOwner;
use crate::world::entity::explosive::fused::primed_tnt::{Detonator, PrimedTntBundle};
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::player_action::PlayerWillDestroyBlock;
use bevy_app::Plugin;
use bevy_ecs::message::MessageReader;
use bevy_ecs::query::{Has, With};
use bevy_ecs::system::{Commands, Query};
use bevy_math::DVec3;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::BlockStateId;

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

pub const PROPERTIES: Properties = Properties::new().instant_break().ignited_by_lava();

pub struct TntBlockPlugin;

impl Plugin for TntBlockPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(bevy_app::FixedUpdate, player_will_destroy_tnt);
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
        println!("prime");
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
