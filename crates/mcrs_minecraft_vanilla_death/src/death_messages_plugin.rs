use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_damage::Died;
use mcrs_minecraft_engine_player_vitals::DeathMessage;
use mcrs_minecraft_engine_player_vitals::PlayerDeathSet;

use crate::announce_deaths::announce_deaths;
use crate::describe_player_deaths::describe_player_deaths;

#[derive(Clone, Copy, Debug, Default)]
pub struct DeathMessagesPlugin;

impl Plugin for DeathMessagesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Died>()
            .add_message::<DeathMessage>()
            .add_systems(
                FixedUpdate,
                (
                    describe_player_deaths
                        .run_if(on_message::<Died>)
                        .in_set(PlayerDeathSet::Describe),
                    announce_deaths
                        .run_if(on_message::<DeathMessage>)
                        .in_set(PlayerDeathSet::Announce),
                ),
            );
    }
}
