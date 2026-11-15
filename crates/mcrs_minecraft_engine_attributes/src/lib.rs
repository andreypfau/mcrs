//! The attribute instances of the players of a dimension world (`attributes.md`, section 3): one
//! edit message, one apply system, and one message for each instance that an edit changed.

mod apply_player_attribute_edits;
mod attribute_changed;
mod attribute_edit;
mod attribute_edits;
mod attribute_target;
mod attributes;
mod player_attribute_edits;
mod player_attributes_plugin;

pub use attribute_changed::AttributeChanged;
pub use attribute_edit::AttributeEdit;
pub use attribute_edits::AttributeEdits;
pub use attribute_target::AttributeTarget;
pub use attributes::Attributes;
pub use player_attribute_edits::PlayerAttributeEdits;
pub use player_attributes_plugin::PlayerAttributesPlugin;
