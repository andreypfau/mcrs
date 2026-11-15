//! The attribute instances of the mob rows of a dimension world (`attributes.md`, section 3):
//! the apply system of their edits, and the motion values that a changed instance writes into
//! its row.

mod apply_row_attribute_edits;
mod attribute_changed;
mod attribute_edit;
mod attribute_edits;
mod attribute_target;
mod forget_removed_rows;
mod mob_attributes_plugin;
mod motion_field;
mod project_row_motion;
mod row_attributes;

pub use attribute_changed::AttributeChanged;
pub use attribute_edit::AttributeEdit;
pub use attribute_edits::AttributeEdits;
pub use attribute_target::AttributeTarget;
pub use mob_attributes_plugin::MobAttributesPlugin;
pub use row_attributes::RowAttributes;
