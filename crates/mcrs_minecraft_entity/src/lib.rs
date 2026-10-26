pub mod attribute;
#[rustfmt::skip]
pub mod keys;
pub mod spawn;
pub mod spawn_condition;
pub mod variant;
pub mod villager;
mod wire;

pub use attribute::AttributeDefinition;

pub const PLAYER_WIDTH: f64 = 0.6;
pub const PLAYER_HEIGHT: f64 = 1.8;
pub const PLAYER_EYE_HEIGHT: f64 = 1.62;
