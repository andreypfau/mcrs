use crate::world::block::behaviour::Properties;
use crate::world::block::{Block, BlockState};
use crate::world::entity::player::ability::InstantBuild;
use crate::world::entity::player::player_action::PlayerWillDestroyBlock;
use bevy_app::Plugin;
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Query;
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
    player: Query<(&InstantBuild)>,
) {
    messages.read().for_each(|event| {
        if event.block_state != UNSTABLE_STATE.id {
            return;
        }
        let instant_build = player
            .get(event.player)
            .map(|i| **i)
            .unwrap_or(*InstantBuild::default());
        if instant_build {
            return;
        }
        println!("prime");
    });
}
