//! The entity attributes, their modifier operations and the computation of an attribute value.
#![no_std]

// std: the codecs of mcrs_minecraft_wire write to `std::io::Write`, and the lookup of an
// attribute by location is a std `HashMap`.
#[cfg(any(feature = "std", feature = "wire"))]
extern crate std;

extern crate alloc;

mod attribute_bases;
mod attribute_definition;
mod attribute_error;
mod attribute_instances;
#[rustfmt::skip]
pub mod attribute_keys;
mod attribute_mask;
mod attribute_modifier;
mod attribute_set;
mod fold;
mod home_slot;
mod instance;
mod instance_change;
mod instance_edit;
mod instance_modifier;
mod modifier;
mod modifier_kind;
mod modifier_operation;
#[rustfmt::skip]
pub mod registry_keys;
mod unknown_operation;
mod value_range;

pub use attribute_bases::AttributeBases;
pub use attribute_bases::TypeBase;
pub use attribute_definition::AttributeDefinition;
pub use attribute_error::AttributeError;
pub use attribute_instances::AttributeInstances;
pub use attribute_mask::AttributeMask;
pub use attribute_modifier::AttributeModifier;
pub use attribute_set::AttributeSet;
pub use fold::fold;
pub use home_slot::HomeSlot;
pub use instance::Instance;
pub use instance_change::InstanceChange;
pub use instance_edit::InstanceEdit;
pub use instance_modifier::InstanceModifier;
pub use modifier::Modifier;
pub use modifier_kind::ModifierKind;
pub use modifier_operation::ModifierOperation;
pub use unknown_operation::UnknownOperation;
pub use value_range::ValueRange;
