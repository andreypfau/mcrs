//! The attribute instances of the players and the mob rows of a dimension world
//! (`attributes.md`, section 3): one edit message, one apply system, and one message for each
//! instance that an edit changed.

mod apply_attribute_edits;
mod attribute_changed;
mod attribute_edit;
mod attribute_target;
mod attributes;
mod entity_attributes_plugin;
mod forget_removed_rows;
mod instance_edit;
mod row_attributes;

pub use attribute_changed::AttributeChanged;
pub use attribute_edit::AttributeEdit;
pub use attribute_target::AttributeTarget;
pub use attributes::Attributes;
pub use entity_attributes_plugin::AttributeEdits;
pub use entity_attributes_plugin::EntityAttributesPlugin;
pub use instance_edit::InstanceEdit;
pub use row_attributes::RowAttributes;
