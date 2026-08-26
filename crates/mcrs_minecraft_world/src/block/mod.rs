use mcrs_minecraft_core::tag::key::TaggedRegistry;

pub mod definition;
pub mod tags;

/// The block registry, as the tag system names it. The blocks themselves live
/// in the definition corpus and are addressed by their index in it, so this
/// type carries no value — it only says which registry a `TagKey` belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Block {}

impl TaggedRegistry for Block {
    const REGISTRY_PATH: &'static str = "block";
}
