use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::GuiRect;
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_creative_content::SearchQuery;
use mcrs_minecraft_engine_inventory_menu::{MenuParts, Menus};
use mcrs_minecraft_engine_item::{SlotRef, Slots, Stacks};
use mcrs_minecraft_entity_attribute::AttributeInstances;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_tooltip::TooltipView;
use mcrs_minecraft_menu_geometry::{CreativeGeometry, CreativeHit, CreativeTabKind};
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_text::{Color, IntoText, Text};

use crate::creative_screen_draw::{self, CreativeLayer, ENTRY_SIZE};
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_config::GuiConfig;
use crate::gui_layer::GuiLayer;
use crate::gui_pointer::GuiPointer;
use crate::gui_size::GuiSize;
use crate::layer_due;
use crate::menu_item_tooltip::{
    self, HoveredItem, HoveredStack, LineSources, MenuItemTooltip, TooltipSlot, TooltipStyle,
};
use crate::tooltip::Tooltip;
use mcrs_minecraft_font::font::Font;
use mcrs_minecraft_font::language::ClientLanguage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddedParts {
    None,
    Tabs,
    TabsAndTags { entry: usize },
}

#[cfg_attr(mcrs_lint, mcrs::alloc_sites(13))]
pub(crate) fn request_creative_item_tooltip(
    player: Query<(&PlayerHolders, &Menus, Option<&AttributeInstances>), With<Player>>,
    menus: Query<(&MenuLayout, &MenuParts)>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    items: Res<ItemTable>,
    registries: Option<Res<CurrentRegistries>>,
    language: Option<Res<ClientLanguage>>,
    atlas: Option<Res<GuiAtlasLayout>>,
    font: Option<Res<Font>>,
    config: Res<GuiConfig>,
    creative: Res<CreativeScreenState>,
    content: Res<CreativeTabContent>,
    time: Res<Time<Real>>,
    (size, pointer): (Res<GuiSize>, Res<GuiPointer>),
    mut layer: Single<(&mut Tooltip, &mut GuiLayer), With<CreativeLayer>>,
    mut tooltip: Local<MenuItemTooltip>,
) {
    let (
        Some(language),
        Some(atlas),
        Some(font),
        Some(registries),
        Ok((player, player_menus, instances)),
    ) = (language, atlas, font, registries, player.single())
    else {
        tooltip.leave();
        return;
    };
    if language.is_changed()
        || items.is_changed()
        || registries.is_changed()
        || atlas.is_changed()
        || font.is_changed()
        || creative.is_changed()
        || content.is_changed()
    {
        tooltip.forget();
    }
    let occupant = |slot: SlotRef| holders.get(slot.holder).ok()?.get(usize::from(slot.index));
    let cursor = SlotRef {
        holder: player.cursor,
        index: 0,
    };
    let pointer = pointer.point();
    let window = CreativeGeometry::window(size.width, size.height);
    let kind = creative.selected_tab().kind;
    let hit = CreativeGeometry::hit(window, pointer, kind, content.visible_tabs());
    let menu = player_menus.open();
    let hovered = menu
        .filter(|_| occupant(cursor).is_none())
        .and_then(|menu| {
            let (stack, slot) = match hit {
                CreativeHit::GridSlot(slot) => (
                    HoveredStack::Value(creative.grid_stack(slot, &content)?),
                    TooltipSlot::Grid(slot),
                ),
                _ => {
                    let entry = hit.menu_index()?;
                    let (layout, parts) = menus.get(menu).ok()?;
                    let held = layout.entry(entry).and_then(|slot| parts.slot(slot))?;
                    (
                        HoveredStack::Entity(occupant(held)?),
                        TooltipSlot::Entry(entry),
                    )
                }
            };
            let corner = creative_screen_draw::slot_corner(hit)?;
            Some(HoveredItem {
                menu,
                slot,
                stack,
                bounds: GuiRect::new(
                    window.x.saturating_add(corner.x),
                    window.y.saturating_add(corner.y),
                    ENTRY_SIZE,
                    ENTRY_SIZE,
                ),
            })
        });
    let Some(hovered) = hovered else {
        tooltip.leave();
        return;
    };
    let grid = matches!(hit, CreativeHit::GridSlot(_));
    let view = if grid {
        TooltipView::Creative
    } else {
        TooltipView::Standard
    };
    let added = added_parts(hit, kind, &creative, &content);
    let style = TooltipStyle {
        view,
        detail: menu_item_tooltip::tooltip_detail(&config),
        attack: menu_item_tooltip::player_attack_bases(instances),
    };
    let sources = LineSources {
        items: &items,
        registries: &registries,
        language: &language,
        font: &font,
    };
    let search_text = creative.search_box().text();
    tooltip.follow(hovered, style, &mut stacks, &sources, |stack, parts| {
        push_added_parts(stack, added, &content, search_text, parts);
    });
    let now_ms = layer_due::gui_clock(&time);
    let (request, layer) = &mut *layer;
    if let Some(at) = tooltip.request(hovered, now_ms, pointer, *size, request) {
        layer.redraw_at(at);
    }
}

fn added_parts(
    hit: CreativeHit,
    kind: CreativeTabKind,
    creative: &CreativeScreenState,
    content: &CreativeTabContent,
) -> AddedParts {
    match (hit, kind) {
        (CreativeHit::GridSlot(_), CreativeTabKind::Items) => AddedParts::None,
        (CreativeHit::GridSlot(slot), CreativeTabKind::Search)
            if creative.search_box().text().starts_with('#') =>
        {
            creative
                .grid_search_entry(slot, content)
                .map_or(AddedParts::Tabs, |entry| AddedParts::TabsAndTags { entry })
        }
        _ => AddedParts::Tabs,
    }
}

fn push_added_parts(
    stack: &ItemStack,
    added: AddedParts,
    content: &CreativeTabContent,
    search_text: &str,
    parts: &mut Vec<Text>,
) {
    if added == AddedParts::None {
        return;
    }
    let tabs = content
        .tabs_holding(stack)
        .map(|tab| Text::translate(tab.tab().title_key, Vec::new()).color(Color::BLUE));
    parts.extend(tabs);
    if let AddedParts::TabsAndTags { entry } = added {
        let query = SearchQuery::new(search_text);
        let tags = content
            .search_index()
            .matching_tags(entry, &query)
            .map(|tag| Text::text(format!("#{tag}")).color(Color::DARK_PURPLE));
        parts.extend(tags);
    }
}
