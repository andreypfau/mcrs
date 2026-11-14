//! The modifier operations of entity attributes and the computation of an attribute value.
#![no_std]

// std: the codecs of mcrs_minecraft_wire write to `std::io::Write`.
#[cfg(feature = "wire")]
extern crate std;

mod attribute_modifier;
mod fold;
mod home_slot;
mod modifier;
mod modifier_operation;
mod unknown_operation;
mod value_range;

pub use attribute_modifier::AttributeModifier;
pub use fold::fold;
pub use home_slot::HomeSlot;
pub use modifier::Modifier;
pub use modifier_operation::ModifierOperation;
pub use unknown_operation::UnknownOperation;
pub use value_range::ValueRange;
