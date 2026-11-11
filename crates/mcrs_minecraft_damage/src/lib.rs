//! The hit procedure of a living entity (`docs/spec/entity-damage.md`, DMG-HIT, DMG-DTH) over its
//! [`Living`](mcrs_minecraft_physics::Living) state and its
//! [`HitRecord`](mcrs_minecraft_physics::HitRecord). A host calls it from the pass or from a
//! command and tells the viewers what the [`HitOutcome`] says.

mod attributes;
mod hit;
mod hit_outcome;
mod hurt;
mod knockback;
mod reduction;
mod source;

pub use attributes::{AttributeInstance, AttributeModifier, AttributeOperation};
pub use hit::Hit;
pub use hit_outcome::HitOutcome;
pub use hurt::hurt;
pub use hurt::preview;
pub use knockback::compute_knockback;
pub use reduction::{apply_absorption_and_health, reduce_by_armor, reduce_by_resistance};
pub use source::DamageSource;
