mod attribute_base_table;
mod client_motion;
pub mod damage_type;
mod entity_type_data;
mod life;
mod mob_data;
mod precise_sync;
mod sprint_modifier;
mod variant_indices;
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
pub mod damage_type_keys;
#[rustfmt::skip]
pub mod damage_type_tag_keys;
#[rustfmt::skip]
pub mod entity_type_keys;
#[rustfmt::skip]
pub mod entity_type_tag_keys;
#[rustfmt::skip]
pub mod frog_variant_keys;
#[rustfmt::skip]
pub mod mob_effect_keys;
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

pub use attribute_base_table::attribute_bases;
pub use client_motion::ClientMotion;
pub use entity_type_data::EntityTypeData;
pub use life::Life;
pub use mob_data::MobData;
pub use precise_sync::PreciseSync;
pub use sprint_modifier::SPRINT_MODIFIER;
pub use variant_indices::VariantIndices;

mcrs_minecraft_registries::resolve_as_is!(crate::damage_type::DamageType);
