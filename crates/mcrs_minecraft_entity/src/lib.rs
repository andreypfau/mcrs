pub mod attribute;
#[rustfmt::skip]
pub mod attribute_keys;
#[rustfmt::skip]
pub mod cat_sound_variant_keys;
#[rustfmt::skip]
pub mod cat_variant_keys;
#[rustfmt::skip]
pub mod chicken_sound_variant_keys;
#[rustfmt::skip]
pub mod chicken_variant_keys;
#[rustfmt::skip]
pub mod cow_sound_variant_keys;
#[rustfmt::skip]
pub mod cow_variant_keys;
#[rustfmt::skip]
pub mod entity_type_keys;
#[rustfmt::skip]
pub mod entity_type_tag_keys;
#[rustfmt::skip]
pub mod frog_variant_keys;
#[rustfmt::skip]
pub mod pig_sound_variant_keys;
#[rustfmt::skip]
pub mod pig_variant_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod spawn_condition_type_keys;
#[rustfmt::skip]
pub mod structure_keys;
#[rustfmt::skip]
pub mod structure_tag_keys;
#[rustfmt::skip]
pub mod villager_profession_keys;
#[rustfmt::skip]
pub mod villager_type_keys;
#[rustfmt::skip]
pub mod wolf_sound_variant_keys;
#[rustfmt::skip]
pub mod wolf_variant_keys;
#[rustfmt::skip]
pub mod zombie_nautilus_variant_keys;
pub mod spawn;
pub mod spawn_condition;
pub mod variant;
pub mod villager;
mod wire;

pub use attribute::AttributeDefinition;

pub const PLAYER_WIDTH: f64 = 0.6;
pub const PLAYER_HEIGHT: f64 = 1.8;
pub const PLAYER_EYE_HEIGHT: f64 = 1.62;
