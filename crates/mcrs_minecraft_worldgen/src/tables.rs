use crate::bevy::TemplateAsset;
use bevy_asset::{AssetServer, Handle};
use mcrs_minecraft_core::ResourceLocation;

/// The loaded template at `<namespace>/structure/<path>.nbt`, if one was
/// requested and the file exists.
pub fn template_handle(
    asset_server: &AssetServer,
    id: &ResourceLocation,
) -> Option<Handle<TemplateAsset>> {
    asset_server.get_handle(template_path(id))
}

pub fn template_path(id: &ResourceLocation) -> String {
    format!("{}/structure/{}.nbt", id.namespace(), id.path())
}
