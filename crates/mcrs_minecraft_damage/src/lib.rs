//! The hit procedure of a living entity (`docs/spec/entity-damage.md`, DMG-HIT, DMG-DTH) over its
//! [`Living`](mcrs_minecraft_physics::Living) state and its
//! [`HitRecord`](mcrs_minecraft_physics::HitRecord). A host calls it from the pass or from a
//! command and tells the viewers what the [`HitOutcome`] says.

mod damage_rules;
mod defence;
mod effects;
mod environment;
mod experience;
mod food;
mod hit;
mod hit_outcome;
mod hurt;
mod knockback;
mod reduction;
mod source;

pub use damage_rules::{DamageTypeFlags, DamageTypeRules};
pub use defence::Defence;
pub use effects::{ActiveEffect, EffectAction, Effects};
pub use environment::{
    FROZEN_TICKS, border_damage, fall_damage, ignite, is_in_void, tick_air, tick_fire, tick_freeze,
};
pub use experience::{Experience, points_for_level, total_points_for_level};
pub use food::{Food, FoodTick};
pub use hit::Hit;
pub use hit_outcome::HitOutcome;
pub use hurt::hurt;
pub use hurt::preview;
pub use knockback::compute_knockback;
pub use mcrs_minecraft_entity::attribute_instance::{
    AttributeInstance, AttributeModifier, AttributeOperation, fold_attribute,
};
pub use reduction::{apply_absorption_and_health, reduce_by_armor, reduce_by_resistance};
pub use source::DamageSource;
