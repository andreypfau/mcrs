//! Combat, health, damage handling, hunger and environmental effects as Bevy systems.

mod combat_plugin;
mod environment;
mod events;
mod health;
mod hunger;
mod life_cycle;

pub use combat_plugin::CombatPlugin;
pub use environment::{AirSupply, FireState, compute_fall_damage};
pub use events::{DamageDealt, DamageRequest, DeathEvent, RespawnRequest};
pub use health::{Armor, ArmorToughness, DamageCooldown, Health, KnockbackResistance};
pub use hunger::FoodData;
pub use life_cycle::{DeathState, process_damage_requests, tick_cooldowns, tick_death};
