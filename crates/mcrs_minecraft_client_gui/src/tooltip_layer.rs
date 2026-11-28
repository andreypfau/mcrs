use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_band::GuiBand;
use crate::gui_layer::GuiLayer;
use crate::gui_size::GuiSize;
use crate::tooltip::{self, Tooltip};
use bevy::prelude::*;
use mcrs_minecraft_font::font::Font;

#[derive(Component, Default)]
#[require(GuiLayer = GuiLayer::new(GuiBand::Overlay, 0))]
pub struct TooltipLayer;

pub(crate) fn draw_tooltip(
    atlas: Option<Res<GuiAtlasLayout>>,
    font: Option<Res<Font>>,
    size: Res<GuiSize>,
    tooltips: Query<(&GuiLayer, &Tooltip), Without<TooltipLayer>>,
    mut layer: Single<&mut GuiLayer, With<TooltipLayer>>,
) {
    let out = layer.redraw();
    let (Some(atlas), Some(font)) = (atlas, font) else {
        return;
    };
    let first = tooltips
        .iter()
        .filter(|(_, tooltip)| tooltip.lines().is_some())
        .min_by_key(|(layer, _)| layer.order());
    let Some((_, request)) = first else {
        return;
    };
    tooltip::draw(request, *size, &atlas, &font, out);
}
