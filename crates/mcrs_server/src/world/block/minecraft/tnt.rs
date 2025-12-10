use bevy_app::Plugin;
use crate::world::block::behaviour::{BlockBehaviour, Properties};
use crate::world::entity::player::ability::InstantBuild;
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Query;
use mcrs_protocol::BlockStateId;
use crate::world::entity::player_action::PlayerWillDestroyBlock;

pub const PROPERTIES: Properties = Properties::new().instant_break().ignited_by_lava();

pub const DEFAULT_STATE: BlockStateId = BlockStateId(2141);
pub const UNSTABLE_STATE: BlockStateId = BlockStateId(2140);

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
        if event.block_state != UNSTABLE_STATE {
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
