//! The ingredient and the keys of the recipe registries.
// std: mcrs_minecraft_wire

#[rustfmt::skip]
pub mod recipe_book_category_keys;
#[rustfmt::skip]
pub mod recipe_display_keys;
#[rustfmt::skip]
pub mod recipe_serializer_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod slot_display_keys;

mod ingredient;

pub use ingredient::Ingredient;
