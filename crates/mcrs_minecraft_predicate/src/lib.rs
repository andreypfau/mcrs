//! The keys of the predicate registries, the view of the world that a predicate reads, and the
//! location, block, fluid, light, distance and item predicates.
// std: mcrs_minecraft_item

#[rustfmt::skip]
pub mod custom_stat_keys;
#[rustfmt::skip]
pub mod entity_sub_predicate_type_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod stat_type_keys;

mod baked_block_predicate;
mod baked_collection;
mod baked_component_predicate;
mod baked_enchantment_predicate;
mod baked_fluid_predicate;
mod baked_item_predicate;
mod baked_location_predicate;
mod baked_modifier_predicate;
mod component_holder;
mod distance_predicate;
mod exact_component;
mod fluid_predicate;
mod id_bits;
mod item_subject;
mod light_predicate;
mod location_predicate;
mod location_view;
mod no_world;
mod predicate_issue;
mod predicate_scratch;
mod property_order;
mod smoke_table;

pub use baked_item_predicate::BakedItemPredicate;
pub use baked_location_predicate::BakedLocationPredicate;
pub use distance_predicate::DistancePredicate;
pub use fluid_predicate::FluidPredicate;
pub use item_subject::ItemSubject;
pub use light_predicate::LightPredicate;
pub use location_predicate::LocationPredicate;
pub use location_view::LocationView;
pub use no_world::NoWorld;
pub use predicate_issue::PredicateIssue;
pub use predicate_scratch::PredicateScratch;
pub use smoke_table::SmokeTable;
