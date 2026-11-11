use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;

use crate::events::{DamageDealt, DamageRequest, DeathEvent, RespawnRequest};
use crate::life_cycle::{process_damage_requests, tick_cooldowns, tick_death};

/// The Bevy plugin providing health, combat, damage handling and entity life cycle.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageRequest>();
        app.add_message::<DamageDealt>();
        app.add_message::<DeathEvent>();
        app.add_message::<RespawnRequest>();

        app.add_systems(
            FixedUpdate,
            (process_damage_requests, tick_cooldowns, tick_death).chain(),
        );
    }
}
