use bevy::math::Vec2;
use bevy::prelude::*;
use mcrs_minecraft_client_widget::{GuiExtent, GuiPoint, GuiRect};
use mcrs_minecraft_client_world::screen::{Screen, ScreenMessage};
use mcrs_minecraft_client_world::world_list::WorldTypes;

use crate::font::Font;
use crate::gui_band::GuiBand;
use crate::gui_layer::GuiLayer;
use crate::gui_quad::WHITE;
use crate::gui_size::GuiSize;
use crate::irect::irect;
use crate::language::ClientLanguage;
use crate::outlined_box::draw_outlined_box;
use crate::text_draw::draw_str;

const PADDING: i32 = 12;
const BOX_HEIGHT: i32 = 33;
const BELOW_CENTRE: i32 = 4;
const OUTLINE: u32 = 0xFFA0_A0A0;

#[derive(Component, Default)]
#[require(GuiLayer = GuiLayer::new(GuiBand::Screen, 1))]
pub struct MessageLayer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MessageBox {
    pub(crate) rect: GuiRect,
    pub(crate) text: GuiPoint,
}

pub(crate) fn message_key(message: ScreenMessage) -> &'static str {
    match message {
        ScreenMessage::ReadingWorldData => "selectWorld.data_read",
        ScreenMessage::PreparingWorldCreation => "createWorld.preparing",
        ScreenMessage::SavingWorld => "menu.savingLevel",
    }
}

pub(crate) fn message_box(screen: GuiExtent, text_width: i32) -> MessageBox {
    let width = text_width.saturating_add(2 * PADDING);
    let x = screen.width / 2 - width / 2;
    let y = screen.height / 2 - BELOW_CENTRE;
    MessageBox {
        rect: GuiRect::new(x, y, width, BOX_HEIGHT),
        text: GuiPoint::new(x + PADDING, y + PADDING),
    }
}

pub(crate) fn draw(
    screen: Res<Screen>,
    types: Option<Res<WorldTypes>>,
    font: Option<Res<Font>>,
    language: Option<Res<ClientLanguage>>,
    size: Res<GuiSize>,
    mut layer: Single<&mut GuiLayer, With<MessageLayer>>,
) {
    let out = layer.redraw();
    let message = match *screen {
        Screen::Message(message) => message,
        Screen::CreateWorld(_) if types.is_some_and(|types| *types == WorldTypes::Loading) => {
            ScreenMessage::PreparingWorldCreation
        }
        _ => return,
    };
    let (Some(font), Some(language)) = (font, language) else {
        return;
    };
    let font = &*font;
    let text = language.get(message_key(message));
    let extent = GuiExtent::new(size.width, size.height);
    let placed = message_box(extent, font.str_width(text));
    draw_outlined_box(irect(placed.rect), OUTLINE, out);
    let origin = Vec2::new(placed.text.x as f32, placed.text.y as f32);
    draw_str(font, origin, text, WHITE, false, out);
}
