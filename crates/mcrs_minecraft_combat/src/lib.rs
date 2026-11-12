//! Combat, health, damage handling, hunger, status effects, experience,
//! projectiles, explosions, equipment tracking and environmental effects as Bevy systems.

mod combat_plugin;
mod damage_rules;
mod death_drops;
mod effects;
mod environment;
mod equipment;
mod events;
mod experience;
mod explosions;
mod health;
mod hunger;
mod life_cycle;
mod position;
mod projectiles;
mod runtime;
mod wire;

pub use combat_plugin::{CombatPlugin, CombatSet};
pub use damage_rules::DamageTypeRulesResource;
pub use death_drops::{DropInventoryOnDeathEvent, handle_death_drops};
pub use effects::{
    ActiveEffect, ActiveEffects, ApplyEffectEvent, MobEffect, RemoveEffectEvent,
    handle_apply_effect_events, tick_active_effects,
};
pub use environment::{
    AirSupply, FireState, FreezeState, WorldBorderState, compute_fall_damage, is_in_void,
};
pub use equipment::{EntityEquipment, EquipmentChangedEvent, track_equipment_changes};
pub use events::{DamageDealt, DamageRequest, DeathEvent, RespawnRequest};
pub use experience::{
    ExperienceGainedEvent, ExperienceOrb, PlayerExperience, pickup_experience_orbs,
    points_for_level, total_points_for_level,
};
pub use explosions::{
    ExplosionOccurred, ExplosionRequest, IgniteTntRequest, PrimedTnt, ignite_tnt_blocks,
    process_explosions, tick_primed_tnt,
};
pub use health::{
    Armor, ArmorToughness, AttackDamage, DamageCooldown, Health, KnockbackResistance,
};
pub use hunger::FoodData;
pub use life_cycle::{DeathState, process_damage_requests, tick_cooldowns, tick_death};
pub use mcrs_minecraft_damage::damage_types;
pub use mcrs_minecraft_damage::{DamageSource, DamageTypeFlags, DamageTypeRules, Hit};
pub use position::CombatPosition;
pub use projectiles::{Projectile, ProjectileHitEvent, ProjectileKind, tick_projectiles};
pub use runtime::CombatRuntimePlugin;
pub use wire::{
    WireEntity, WireRegistry, attach_wire_entities, forget_wire_entities, sync_wire_entities,
};
