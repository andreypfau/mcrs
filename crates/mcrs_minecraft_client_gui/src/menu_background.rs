use bevy::math::{IVec2, UVec2};
use mcrs_minecraft_menu_geometry::{BackgroundPart, CreativeBackground, MenuGeometry};
use mcrs_minecraft_menu_key::menu_keys::MenuType;
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

fn background_parts() -> impl Iterator<Item = BackgroundPart> {
    MenuType::ALL
        .iter()
        .filter_map(|kind| MenuGeometry::of(*kind))
        .chain([MenuGeometry::INVENTORY])
        .flat_map(|geometry| geometry.background())
        .chain(
            [
                CreativeBackground::Items,
                CreativeBackground::ItemSearch,
                CreativeBackground::Inventory,
            ]
            .map(CreativeBackground::part),
        )
}

pub(crate) fn menu_backgrounds() -> impl Iterator<Item = ResourceLocation<'static>> {
    background_parts().map(|part| part.texture)
}

pub(crate) fn background_extent(texture: ResourceLocation<'static>) -> UVec2 {
    background_parts()
        .filter(|part| part.texture == texture)
        .map(|part| {
            IVec2::new(
                part.source.x.saturating_add(part.source.width),
                part.source.y.saturating_add(part.source.height),
            )
        })
        .fold(IVec2::ZERO, IVec2::max)
        .as_uvec2()
}
