use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::query::{Changed, With, Without};
use bevy_ecs::system::{Commands, Populated};
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::HiddenPlayer;
use mcrs_minecraft_engine::player_signal::PlayerSignal;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_replication::MobSignal;

const DEATH_TICKS: u8 = 20;

pub(crate) fn hide_dead_players(
    players: Populated<(Entity, &Vitals), (With<Player>, Without<HiddenPlayer>, Changed<Vitals>)>,
    mut signals: MessageWriter<PlayerSignal>,
    mut commands: Commands,
) {
    for (player, vitals) in &players {
        if vitals.death_time.is_some_and(|ticks| ticks >= DEATH_TICKS) {
            signals.write(PlayerSignal::new(player, MobSignal::Removed));
            commands.entity(player).insert(HiddenPlayer);
        }
    }
}
