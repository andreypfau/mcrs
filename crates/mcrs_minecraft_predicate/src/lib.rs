#[rustfmt::skip]
pub mod keys;
pub mod damage;
pub mod entity;
pub mod location;
pub mod player;
pub mod slots;

pub use damage::{DamageSourcePredicate, TagPredicate};
pub use entity::EntityPredicate;
pub use location::LocationPredicate;

pub(crate) fn is_any_int(
    bounds: &mcrs_minecraft_item::component::common::MinMaxBounds<i32>,
) -> bool {
    bounds.is_any()
}

pub(crate) fn is_any_double(
    bounds: &mcrs_minecraft_item::component::common::MinMaxBounds<f64>,
) -> bool {
    bounds.is_any()
}
