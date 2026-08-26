use mcrs_core::tag::key::TaggedRegistry;

pub mod tnt;

/// The block registry, as the tag system names it. Blocks themselves come from
/// the definition corpus, so this type carries no value — it only says which
/// registry a `TagKey` belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Block {}

impl TaggedRegistry for Block {
    const REGISTRY_PATH: &'static str = "block";
}

pub struct MinecraftBlockPlugin;

impl bevy_app::Plugin for MinecraftBlockPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_plugins(tnt::TntBlockPlugin);
    }
}
