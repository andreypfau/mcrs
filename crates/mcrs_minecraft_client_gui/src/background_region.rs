use mcrs_minecraft_resource::ResourceLocation;

const TEXTURE_FOLDER: &str = "textures/gui/";
const TEXTURE_SUFFIX: &str = ".png";

pub(crate) fn background_region(texture: ResourceLocation<'static>) -> Option<&'static str> {
    if texture.namespace() != "minecraft" {
        return None;
    }
    texture
        .path()
        .strip_prefix(TEXTURE_FOLDER)?
        .strip_suffix(TEXTURE_SUFFIX)
}
