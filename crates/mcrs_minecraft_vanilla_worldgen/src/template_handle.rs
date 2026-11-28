use crate::template_asset::TemplateAsset;
use bevy_asset::{AssetServer, Handle};
use mcrs_minecraft_resource::ResourceLocationBuf;

pub fn template_handle(
    asset_server: &AssetServer,
    id: &ResourceLocationBuf,
) -> Option<Handle<TemplateAsset>> {
    asset_server.get_handle(template_path(id))
}

pub fn template_path(id: &ResourceLocationBuf) -> String {
    format!("{}/structure/{}.nbt", id.namespace(), id.path())
}
