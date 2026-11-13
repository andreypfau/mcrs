//! Combat, health, damage handling, hunger, status effects, experience and equipment as Bevy
//! systems.

mod combat_plugin;
mod components;
mod damage_rules;
mod death_drops;
mod effects;
mod equipment;
mod events;
mod experience;
mod hidden;
mod life_cycle;
mod runtime;

pub use combat_plugin::{CombatPlugin, CombatSet};
pub use components::{
    Armor, ArmorToughness, AttackDamage, BaseMaxHealth, Dying, KnockbackResistance,
    PlayerAttributes, PlayerEffects, PlayerEffectsHidden, PlayerEnvironment, PlayerExperience,
    PlayerFood, PlayerHitRecord, PlayerVitals, RespawnPoint,
};
pub use damage_rules::DamageTypeRulesResource;
pub use death_drops::{DropInventoryOnDeathEvent, handle_death_drops};
pub use effects::{
    ActiveEffect, ApplyEffectEvent, MobEffect, RemoveEffectEvent, handle_apply_effect_events,
    tick_active_effects,
};
pub use equipment::EntityEquipment;
pub use events::{
    DamageDealt, DamageRequest, DamageTarget, DeathEvent, KnockbackRequest, RespawnRequest,
};
pub use experience::ExperienceGainedEvent;
pub use life_cycle::{apply_knockback, process_damage_requests, tick_death, tick_player_cooldowns};
pub use mcrs_minecraft_damage::{
    DamageSource, DamageTypeFlags, DamageTypeRules, Effects, Experience, Food, Hit,
    points_for_level, total_points_for_level,
};
pub use mcrs_minecraft_physics::{HitRecord, Vitals};
