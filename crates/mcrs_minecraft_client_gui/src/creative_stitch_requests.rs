use mcrs_minecraft_menu_geometry::{CREATIVE_TABS, CreativeBackground, CreativeGeometry};

use crate::menu_background::background_request;
use crate::stitch_request::StitchRequest;

pub(crate) fn creative_stitch_requests() -> impl Iterator<Item = StitchRequest> {
    CREATIVE_TABS
        .into_iter()
        .flat_map(|tab| [tab.selected_sprite(), tab.unselected_sprite()])
        .chain([
            CreativeGeometry::SCROLLER,
            CreativeGeometry::SCROLLER_DISABLED,
        ])
        .map(StitchRequest::Sprite)
        .chain(
            [
                CreativeBackground::Items,
                CreativeBackground::ItemSearch,
                CreativeBackground::Inventory,
            ]
            .into_iter()
            .filter_map(|background| background_request(background.part())),
        )
}
