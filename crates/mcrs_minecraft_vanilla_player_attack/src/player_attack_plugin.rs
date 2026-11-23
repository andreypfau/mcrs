use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemCondition;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine_damage::DamageSet;
use mcrs_minecraft_engine_damage::Damaged;
use mcrs_minecraft_engine_damage::PendingHits;
use mcrs_minecraft_engine_world::entity::player::Player;

use crate::AttackCounter;
use crate::attack_counter::reset_on_punch;
use crate::attack_knockbacks::AttackKnockbacks;
use crate::attack_knockbacks::has_attack_knockbacks;
use crate::attack_packet::on_attack;
use crate::knock_back::knock_back;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerAttackPlugin;

impl Plugin for PlayerAttackPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app.try_register_required_components::<Player, AttackCounter>() {
            tracing::error!("a player does not require its attack counter: {error}");
        }
        app.init_resource::<PendingHits>()
            .init_resource::<AttackKnockbacks>()
            .add_message::<OutboundPlayerPacket>()
            .add_message::<Damaged>()
            .add_observer(on_attack)
            .add_observer(reset_on_punch)
            .add_systems(
                FixedUpdate,
                knock_back
                    .run_if(has_attack_knockbacks.or_else(on_message::<Damaged>))
                    .in_set(DamageSet::Report),
            );
    }
}
