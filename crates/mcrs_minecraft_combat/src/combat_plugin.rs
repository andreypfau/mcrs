use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::death_drops::{DropInventoryOnDeathEvent, handle_death_drops};
use crate::effects::{
    ApplyEffectEvent, RemoveEffectEvent, handle_apply_effect_events, tick_active_effects,
};
use crate::equipment::{EquipmentChangedEvent, track_equipment_changes};
use crate::events::{DamageDealt, DamageRequest, DeathEvent, RespawnRequest};
use crate::experience::{ExperienceGainedEvent, pickup_experience_orbs};
use crate::explosions::{
    ExplosionOccurred, ExplosionRequest, process_explosions, tick_primed_tnt,
};
use crate::life_cycle::{process_damage_requests, tick_cooldowns, tick_death};
use crate::projectiles::{ProjectileHitEvent, tick_projectiles};

/// The Bevy plugin providing health, combat, status effects, experience,
/// projectiles, explosions, equipment and entity life cycle.
#[derive(Clone, Copy, Debug, Default)]
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        // Register events / messages
        app.add_message::<DamageRequest>();
        app.add_message::<DamageDealt>();
        app.add_message::<DeathEvent>();
        app.add_message::<RespawnRequest>();
        app.add_message::<ApplyEffectEvent>();
        app.add_message::<RemoveEffectEvent>();
        app.add_message::<DropInventoryOnDeathEvent>();
        app.add_message::<EquipmentChangedEvent>();
        app.add_message::<ExperienceGainedEvent>();
        app.add_message::<ProjectileHitEvent>();
        app.add_message::<ExplosionRequest>();
        app.add_message::<ExplosionOccurred>();

        // Register combat systems in FixedUpdate within TickSet::Simulate in a defined sequence
        app.add_systems(
            FixedUpdate,
            (
                tick_primed_tnt,
                process_explosions,
                tick_projectiles,
                handle_apply_effect_events,
                tick_active_effects,
                process_damage_requests,
                tick_cooldowns,
                handle_death_drops,
                tick_death,
                pickup_experience_orbs,
                track_equipment_changes,
            )
                .chain()
                .in_set(TickSet::Simulate),
        );
    }
}
