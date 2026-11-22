use crate::hide_dead_players::hide_dead_players;
use crate::player_invulnerability::{fill_player_invulnerability, invulnerable};
use crate::respawn_players::{HiddenForRespawn, respawn_players, show_respawned_players};
use crate::send_health::send_health;
use crate::send_kill::send_kill;
use crate::signal_hits::signal_player_hits;
use crate::{DeathMessage, PlayerDeathSet};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::schedule::common_conditions::{any_with_component, on_message};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy_ecs::system::{Commands, Query};
use mcrs_minecraft_damage::{Defence, HitRecord, Vitals};
use mcrs_minecraft_engine::{OwnAttributesSet, PoseSystems, max_health};
use mcrs_minecraft_engine_damage::{DamagePlugin, DamageSet, Damaged, Died, RespawnRequest};
use mcrs_minecraft_engine_world::entity::player::{ChangeDimensionRequest, Player};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_game_mode::PlayerGameMode;

#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerVitalsPlugin;

impl Plugin for PlayerVitalsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<DamagePlugin>() {
            app.add_plugins(DamagePlugin);
        }
        app.add_observer(give_spawned_player_vitals);
        app.add_systems(
            FixedUpdate,
            fill_player_invulnerability
                .in_set(TickSet::Simulate)
                .before(DamageSet::Collect),
        );
        app.add_systems(
            FixedUpdate,
            send_health
                .in_set(TickSet::Project)
                .after(DamageSet::Report)
                .after(OwnAttributesSet),
        );
        app.add_systems(
            FixedUpdate,
            signal_player_hits
                .run_if(on_message::<Damaged>)
                .in_set(TickSet::Project)
                .after(DamageSet::Report)
                .after(PoseSystems::Publish)
                .after(send_health),
        );
        app.add_message::<DeathMessage>();
        app.configure_sets(
            FixedUpdate,
            PlayerDeathSet::Describe
                .in_set(TickSet::WorldWrite)
                .after(DamageSet::Apply),
        );
        app.add_systems(
            FixedUpdate,
            send_kill
                .run_if(on_message::<Died>.or_else(on_message::<DeathMessage>))
                .in_set(TickSet::Project)
                .after(PlayerDeathSet::Describe)
                .after(DamageSet::Report)
                .after(send_health)
                .before(signal_player_hits),
        );
        app.add_systems(
            FixedUpdate,
            hide_dead_players
                .in_set(TickSet::Project)
                .after(signal_player_hits),
        );
        app.add_message::<RespawnRequest>();
        app.add_message::<ChangeDimensionRequest>();
        app.add_systems(
            FixedUpdate,
            (
                show_respawned_players.run_if(any_with_component::<HiddenForRespawn>),
                respawn_players.run_if(on_message::<RespawnRequest>),
            )
                .chain()
                .in_set(TickSet::WorldWrite)
                .after(DamageSet::Report),
        );
    }
}

fn give_spawned_player_vitals(
    add: On<Add<Player>>,
    attributes: Query<&AttributeInstances>,
    modes: Query<&PlayerGameMode>,
    mut commands: Commands,
) {
    let player = add.event().entity;
    let mut defence = Defence::default();
    defence.invulnerable = modes.get(player).is_ok_and(|mode| invulnerable(mode.0));
    commands.entity(player).insert_if_new((
        Vitals::new(max_health(attributes.get(player).ok())),
        HitRecord::default(),
        defence,
    ));
}
