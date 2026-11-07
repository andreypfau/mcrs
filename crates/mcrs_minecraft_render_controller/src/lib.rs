//! The render controllers of a client entity: the files, their compile against one client entity
//! and the layers that they select in a frame.

// std: mcrs_minecraft_entity_animation

mod bone_pattern;
mod client_entity_resources;
mod layer_choice;
mod render_controller_file;
mod render_controllers;
mod render_query;

pub use bone_pattern::matches_bone;
pub use client_entity_resources::{ClientEntityResources, ControllerEntry};
pub use layer_choice::{LayerChoice, TextureChoice};
pub use render_controller_file::{RenderControllerLibrary, RenderControllerSource};
pub use render_controllers::{CompiledLayer, RenderControllers};
pub use render_query::RenderQuery;
