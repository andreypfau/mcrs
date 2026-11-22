use crate::send_health::send_health;
use crate::send_kill::send_kill;
use crate::signal_hits::signal_player_hits;
use crate::{DeathMessage, PlayerDeathSet};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::lifecycle::Add;
use bevy_ecs::observer::On;
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemCondition};
use bevy_ecs::system::{Commands, Query};
use mcrs_minecraft_damage::{Defence, HitRecord, Vitals};
use mcrs_minecraft_engine::{OwnAttributesSet, PoseSystems};
use mcrs_minecraft_engine_damage::{DamagePlugin, DamageSet, Damaged, Died};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;

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
    }
}

fn give_spawned_player_vitals(
    add: On<Add<Player>>,
    attributes: Query<&AttributeInstances>,
    mut commands: Commands,
) {
    let player = add.event().entity;
    let max_health = attributes.get(player).map_or_else(
        |_| {
            mcrs_minecraft_entity::attribute_bases(EntityType::Player)
                .and_then(|bases| bases.base(Attribute::MaxHealth))
        },
        |instances| instances.value(Attribute::MaxHealth),
    );
    commands.entity(player).insert_if_new((
        Vitals::new(max_health.unwrap_or_default() as f32),
        HitRecord::default(),
        Defence::default(),
    ));
}
