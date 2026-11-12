use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_engine_physics::EntityPass;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::death_drops::{DropInventoryOnDeathEvent, drop_dead_inventories, handle_death_drops};
use crate::effects::{
    ApplyEffectEvent, RemoveEffectEvent, handle_apply_effect_events, tick_active_effects,
};
use crate::equipment::{EquipmentChangedEvent, track_equipment_changes};
use crate::events::{DamageDealt, DamageRequest, DeathEvent, KnockbackRequest, RespawnRequest};
use crate::experience::{ExperienceGainedEvent, pickup_experience_orbs};
use crate::explosions::{
    ExplosionOccurred, ExplosionRequest, IgniteTntRequest, destroy_explosion_blocks,
    ignite_tnt_blocks, process_explosions, tick_primed_tnt,
};
use crate::life_cycle::{apply_knockback, process_damage_requests, tick_cooldowns, tick_death};
use crate::projectiles::{ProjectileHitEvent, tick_projectiles};
use crate::wire::{WireRegistry, attach_wire_entities, forget_wire_entities, sync_wire_entities};

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombatSet {
    Pre,
    World,
    Outcome,
    Progress,
    Post,
    Send,
}

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
        app.add_message::<KnockbackRequest>();
        app.init_resource::<crate::hunger::CombatDifficulty>();
        app.init_resource::<WireRegistry>();
        app.add_message::<ApplyEffectEvent>();
        app.add_message::<RemoveEffectEvent>();
        app.add_message::<DropInventoryOnDeathEvent>();
        app.add_message::<EquipmentChangedEvent>();
        app.add_message::<ExperienceGainedEvent>();
        app.add_message::<ProjectileHitEvent>();
        app.add_message::<ExplosionRequest>();
        app.add_message::<ExplosionOccurred>();
        app.add_message::<IgniteTntRequest>();

        app.configure_sets(
            FixedUpdate,
            (
                CombatSet::Pre,
                CombatSet::World,
                CombatSet::Outcome,
                CombatSet::Post,
                CombatSet::Progress,
                CombatSet::Send,
            )
                .chain()
                .after(EntityPass)
                .in_set(TickSet::Simulate),
        );
        app.add_systems(
            FixedUpdate,
            (
                ignite_tnt_blocks,
                tick_primed_tnt,
                process_explosions,
                destroy_explosion_blocks,
                tick_projectiles,
                handle_apply_effect_events,
                tick_active_effects,
            )
                .chain()
                .in_set(CombatSet::World),
        );
        app.add_systems(
            FixedUpdate,
            (
                process_damage_requests,
                apply_knockback,
                tick_cooldowns,
                handle_death_drops,
                drop_dead_inventories,
                tick_death,
            )
                .chain()
                .in_set(CombatSet::Outcome),
        );
        app.add_systems(
            FixedUpdate,
            (pickup_experience_orbs, track_equipment_changes)
                .chain()
                .in_set(CombatSet::Progress),
        );
        app.add_systems(
            FixedUpdate,
            (
                attach_wire_entities,
                sync_wire_entities,
                forget_wire_entities,
            )
                .chain()
                .in_set(CombatSet::Send),
        );
    }
}
