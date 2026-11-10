//! The hit procedure of a living entity (`docs/spec/entity-damage.md`, DMG-HIT, DMG-DTH) over its
//! [`Living`](mcrs_minecraft_physics::Living) state and its
//! [`HitRecord`](mcrs_minecraft_physics::HitRecord). A host calls it from the pass or from a
//! command and tells the viewers what the [`HitOutcome`] says.

mod hit;
mod hit_outcome;
mod hurt;

pub use hit::Hit;
pub use hit_outcome::HitOutcome;
pub use hurt::hurt;
pub use hurt::preview;
