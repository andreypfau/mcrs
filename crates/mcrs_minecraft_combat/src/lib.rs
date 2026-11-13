//! Combat, health, damage handling, hunger, status effects, experience and equipment as Bevy
//! systems.

mod combat_plugin;
mod damage_rules;
mod death_drops;
mod effects;
mod environment;
mod equipment;
mod events;
mod experience;
mod health;
mod hunger;
mod life_cycle;
mod position;
mod runtime;

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
    ExperienceGainedEvent, PlayerExperience, points_for_level, total_points_for_level,
};
pub use health::{
    Armor, ArmorToughness, AttackDamage, DamageCooldown, Health, KnockbackResistance,
};
pub use hunger::FoodData;
pub use life_cycle::{DeathState, process_damage_requests, tick_cooldowns, tick_death};
pub use mcrs_minecraft_damage::damage_types;
pub use mcrs_minecraft_damage::{DamageSource, DamageTypeFlags, DamageTypeRules, Hit};
pub use position::CombatPosition;
