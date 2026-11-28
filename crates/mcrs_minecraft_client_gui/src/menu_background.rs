use bevy::math::IVec2;
use mcrs_minecraft_menu_geometry::{BackgroundPart, MenuGeometry};
use mcrs_minecraft_menu_key::menu_keys::MenuType;

use crate::stitch_request::StitchRequest;

pub(crate) fn container_backgrounds() -> impl Iterator<Item = StitchRequest> {
    MenuType::ALL
        .iter()
        .filter_map(|kind| MenuGeometry::of(*kind))
        .chain([MenuGeometry::INVENTORY])
        .flat_map(|geometry| geometry.background())
        .filter_map(background_request)
}

pub(crate) fn background_request(part: BackgroundPart) -> Option<StitchRequest> {
    let extent = IVec2::new(
        part.source.x.saturating_add(part.source.width),
        part.source.y.saturating_add(part.source.height),
    )
    .max(IVec2::ZERO)
    .as_uvec2();
    StitchRequest::background(part.texture, extent)
}
