use bevy_app::App;
use bevy_app::Plugin;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_world::entity::player::Player;

use crate::AttackCounter;
use crate::attack_counter::reset_on_punch;
use crate::attack_packet::on_attack;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerAttackPlugin;

impl Plugin for PlayerAttackPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app.try_register_required_components::<Player, AttackCounter>() {
            tracing::error!("a player does not require its attack counter: {error}");
        }
        app.init_resource::<PendingHits>()
            .add_message::<OutboundPlayerPacket>()
            .add_observer(on_attack)
            .add_observer(reset_on_punch);
    }
}
