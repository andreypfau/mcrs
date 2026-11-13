//! The rules of an item entity without Bevy: its counters and lifetime, its start values for
//! each cause, its pickup into a player inventory and its merge with another item entity.
// std: mcrs_minecraft_physics

mod cause;
mod hand_form;
mod item_entity;
mod merge;
mod pickup;
mod start;

pub use cause::Cause;
pub use hand_form::HandVelocity;
pub use hand_form::default_form;
pub use hand_form::hand_form;
pub use item_entity::DEFAULT_HEALTH;
pub use item_entity::ItemEntity;
pub use item_entity::LIFETIME;
pub use item_entity::Lifetime;
pub use item_entity::NEVER_AGES;
pub use item_entity::NEVER_PICKED_UP;
pub use merge::Target;
pub use merge::merge_box;
pub use merge::merge_due;
pub use merge::merge_target;
pub use pickup::can_pick_up;
pub use pickup::pickup_box;
pub use start::Start;
