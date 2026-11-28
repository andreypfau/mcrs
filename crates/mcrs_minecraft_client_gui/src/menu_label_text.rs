use bevy::math::IVec2;
use bevy::prelude::*;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_item::ItemHoverMap;
use mcrs_minecraft_menu_geometry::{MenuGeometry, MenuLabels, TitlePosition};

use crate::gui_quad::GuiQuad;
use crate::irect;
use crate::text_draw::draw_text_without_shadow;
use mcrs_minecraft_font::font::{Font, Span};
use mcrs_minecraft_font::language::ClientLanguage;
use mcrs_minecraft_font::text::text_spans;

pub(crate) const LABEL_COLOR: u32 = 0xFF40_4040;
const INVENTORY_LABEL: &str = "container.inventory";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuLabelText {
    menu: Option<Entity>,
    titled: bool,
    title: Vec<Span>,
    inventory: Vec<Span>,
}

impl MenuLabelText {
    pub fn follow(
        &mut self,
        menu: Entity,
        title: Option<Ref<MenuTitle>>,
        language: Ref<ClientLanguage>,
    ) {
        let stale = self.menu != Some(menu)
            || language.is_changed()
            || self.titled != title.is_some()
            || title.as_ref().is_some_and(DetectChanges::is_changed);
        if !stale {
            return;
        }
        self.menu = Some(menu);
        self.titled = title.is_some();
        self.write(title.as_deref(), &language);
    }

    pub(crate) fn write(&mut self, title: Option<&MenuTitle>, language: &ClientLanguage) {
        self.title = title.map_or_else(Vec::new, |title| match title.0.map(&ItemHoverMap) {
            Ok(text) => text_spans(&text, language, LABEL_COLOR),
            Err(_) => vec![Span::new(language.plain_text(&title.0), LABEL_COLOR)],
        });
        self.inventory = language.translate(INVENTORY_LABEL, LABEL_COLOR, &[]);
    }

    pub(crate) fn draw(
        &self,
        font: &Font,
        top_left: IVec2,
        geometry: &MenuGeometry,
        positions: MenuLabels,
        out: &mut Vec<GuiQuad>,
    ) {
        let title = match positions.title {
            TitlePosition::Left(at) => Some(irect::point(at)),
            TitlePosition::Centred { y } => Some(IVec2::new(
                geometry
                    .width()
                    .saturating_sub(font.width(&self.title))
                    .div_euclid(2),
                y,
            )),
            _ => None,
        };
        if let Some(title) = title {
            draw_text_without_shadow(font, top_left + title, &self.title, out);
        }
        draw_text_without_shadow(
            font,
            top_left + irect::point(positions.player_inventory),
            &self.inventory,
            out,
        );
    }
}
