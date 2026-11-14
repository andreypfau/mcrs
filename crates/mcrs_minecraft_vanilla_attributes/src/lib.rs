//! Vanilla sources of attribute modifiers on the players of a dimension world
//! (`attributes.md`, section 5). Each source is a plugin that a server can leave out.

mod attribute_sources;
mod creative_range_plugin;
mod sprint_attribute_plugin;

pub use attribute_sources::AttributeSources;
pub use creative_range_plugin::CreativeRangePlugin;
pub use sprint_attribute_plugin::SprintAttributePlugin;
