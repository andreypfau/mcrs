use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;

use crate::death_drops::{DropInventoryOnDeathEvent, handle_death_drops};
use crate::effects::{
    ApplyEffectEvent, RemoveEffectEvent, handle_apply_effect_events, tick_active_effects,
};
use crate::equipment::{EquipmentChangedEvent, track_equipment_changes};
use crate::events::{DamageDealt, DamageRequest, DeathEvent, RespawnRequest};
use crate::experience::{ExperienceGainedEvent, pickup_experience_orbs};
use crate::life_cycle::{process_damage_requests, tick_cooldowns, tick_death};
use crate::projectiles::{ProjectileHitEvent, tick_projectiles};

/// The Bevy plugin providing health, combat, status effects, experience,
/// projectiles, equipment and entity life cycle.
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

        // Register combat systems in FixedUpdate
        app.add_systems(
            FixedUpdate,
            (process_damage_requests, tick_cooldowns, tick_death).chain(),
        );
        app.add_systems(
            FixedUpdate,
            (
                tick_active_effects,
                handle_apply_effect_events,
                tick_projectiles,
                pickup_experience_orbs,
                track_equipment_changes,
                handle_death_drops,
            ),
        );
    }
}
