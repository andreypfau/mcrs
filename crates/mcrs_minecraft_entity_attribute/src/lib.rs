#![no_std]

#[cfg(any(feature = "std", feature = "wire"))]
extern crate std;

extern crate alloc;

mod attribute_bases;
mod attribute_definition;
mod attribute_error;
mod attribute_id;
mod attribute_instances;
#[rustfmt::skip]
pub mod attribute_keys;
mod attribute_mask;
mod attribute_modifier;
mod attribute_set;
#[cfg(feature = "wire")]
mod attribute_snapshot;
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
#[cfg(feature = "serde")]
mod saved_attribute;
#[cfg(feature = "serde")]
mod saved_attributes;
#[cfg(feature = "serde")]
mod saved_modifier;
#[cfg(feature = "serde")]
mod tolerant;
mod unknown_attribute;
mod unknown_operation;
mod value_range;

pub use attribute_bases::AttributeBases;
pub use attribute_bases::TypeBase;
pub use attribute_definition::AttributeDefinition;
pub use attribute_error::AttributeError;
pub use attribute_id::AttributeId;
pub use attribute_instances::AttributeInstances;
pub use attribute_mask::AttributeMask;
pub use attribute_modifier::AttributeModifier;
pub use attribute_set::AttributeSet;
#[cfg(feature = "wire")]
pub use attribute_snapshot::AttributeSnapshot;
pub use fold::fold;
pub use home_slot::HomeSlot;
pub use instance::Instance;
pub use instance_change::InstanceChange;
pub use instance_edit::InstanceEdit;
pub use instance_modifier::InstanceModifier;
pub use modifier::Modifier;
pub use modifier_kind::ModifierKind;
pub use modifier_operation::ModifierOperation;
#[cfg(feature = "serde")]
pub use saved_attribute::SavedAttribute;
#[cfg(feature = "serde")]
pub use saved_attributes::SavedAttributes;
#[cfg(feature = "serde")]
pub use saved_modifier::SavedModifier;
pub use unknown_attribute::UnknownAttribute;
pub use unknown_operation::UnknownOperation;
pub use value_range::ValueRange;
