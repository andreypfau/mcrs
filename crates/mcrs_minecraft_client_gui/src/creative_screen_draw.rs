use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::{EditBoxFrame, GuiPoint, GuiRect};
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_client_world::screen::Screen;
use mcrs_minecraft_container_gesture::DragPreview;
use mcrs_minecraft_engine_inventory_menu::{MenuParts, Menus};
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_icon::Overlays;
use mcrs_minecraft_menu_geometry::{
    CreativeGeometry, CreativeHit, CreativeTabIndex, CreativeTabKind, VisibleTabs,
};
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::creative_screen_shown::shows_creative_screen;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;
use crate::edit_box_draw;
use crate::figure_placement::FigurePlacement;
use crate::font::{Font, Span};
use crate::gui_atlas_layout::GuiAtlasLayout;
use crate::gui_band::GuiBand;
use crate::gui_layer::GuiLayer;
use crate::gui_pointer::GuiPointer;
use crate::gui_quad::GuiQuad;
use crate::gui_quad::WHITE;
use crate::gui_size::GuiSize;
use crate::irect;
use crate::item_icons::ItemIcons;
use crate::language::ClientLanguage;
use crate::menu_input;
use crate::menu_label_text::LABEL_COLOR;
use crate::menu_screen::{self, DIM_BOTTOM, DIM_TOP};
use crate::menu_stacks::MenuStacks;
use crate::shown_drag_preview::ShownDragPreview;
use crate::slot_icon::SlotIcon;
use crate::text_draw;
use crate::tooltip::Tooltip;

#[derive(Component, Default)]
#[require(GuiLayer = GuiLayer::new(GuiBand::Screen, 4), Tooltip)]
pub struct CreativeLayer;

const GRID_SLOTS: u16 = 45;
const HOTBAR_COLUMNS: u16 = 9;
const HOTBAR_MENU_START: u16 = 36;
const FIRST_INVENTORY_ENTRY: u16 = 5;
const LAST_INVENTORY_ENTRY: u16 = 45;
pub(crate) const ENTRY_SIZE: i32 = 16;
const HIGHLIGHT_INSET: i32 = 4;
const HIGHLIGHT_SIZE: i32 = 24;
const DESTROY_SLOT_KEY: &str = "inventory.binSlot";

pub(crate) fn draw_creative_screen(
    screen: Option<Res<Screen>>,
    player: Query<(&PlayerHolders, &Menus, Option<&Abilities>), With<Player>>,
    menus: Query<(&MenuLayout, &MenuParts)>,
    holders: Query<&Slots>,
    (creative, content): (Res<CreativeScreenState>, Res<CreativeTabContent>),
    atlas: Option<Res<GuiAtlasLayout>>,
    font: Option<Res<Font>>,
    language: Option<Res<ClientLanguage>>,
    drag: Option<Res<ShownDragPreview>>,
    time: Res<Time<Real>>,
    icons: ItemIcons,
    slot_icons: Query<&'static SlotIcon>,
    mut stacks: Local<MenuStacks>,
    text_tooltip: Res<CreativeTextTooltip>,
    (size, pointer): (Res<GuiSize>, Res<GuiPointer>),
    mut layer: Single<(&mut GuiLayer, &mut Tooltip), With<CreativeLayer>>,
) {
    let (layer, request) = &mut *layer;
    layer.redraw();
    request.clear();
    let (Some(atlas), Some(font), Some(language)) = (atlas, font, language) else {
        return;
    };
    let Ok((player, player_menus, abilities)) = player.single() else {
        return;
    };
    if !screen.is_some_and(|screen| shows_creative_screen(*screen, player_menus, abilities)) {
        return;
    }
    let Some((layout, parts)) = player_menus.open().and_then(|menu| menus.get(menu).ok()) else {
        return;
    };
    stacks.collect(layout, parts, player.cursor, |slot| {
        holders.get(slot.holder).ok()?.get(usize::from(slot.index))
    });
    let no_drag = DragPreview::default();
    let now_ms = menu_input::menu_clock(&time);
    let frame = CreativeFrame {
        state: &creative,
        content: &content,
        stacks: &stacks,
        drag: drag.as_deref().map_or(&no_drag, ShownDragPreview::preview),
        slot_icons: &slot_icons,
        atlas: &atlas,
        font: &font,
        language: &language,
        now_ms,
        size: *size,
        pointer: *pointer,
    };
    creative_screen(&frame, &icons, &text_tooltip, layer, request);
    if creative.selected_tab().kind == CreativeTabKind::Search
        && let Some(at) = creative.search_box().next_blink_ms(now_ms)
    {
        layer.redraw_at(at);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextTarget {
    Tab(CreativeTabIndex),
    DestroySlot,
}

impl TextTarget {
    fn key(self) -> &'static str {
        match self {
            Self::Tab(tab) => tab.tab().title_key,
            Self::DestroySlot => DESTROY_SLOT_KEY,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct CreativeTextTooltip {
    lines: Vec<(TextTarget, Vec<Vec<Span>>)>,
}

pub(crate) fn rebuild_creative_text(
    language: Res<ClientLanguage>,
    mut tooltip: ResMut<CreativeTextTooltip>,
) {
    let targets = CreativeTabIndex::all()
        .map(TextTarget::Tab)
        .chain([TextTarget::DestroySlot]);
    tooltip.lines.clear();
    for target in targets {
        let spans = vec![Span::new(language.get(target.key()), WHITE)];
        tooltip.lines.push((target, vec![spans]));
    }
}

impl CreativeTextTooltip {
    fn request(&self, target: TextTarget, pointer: GuiPointer, request: &mut Tooltip) {
        if let Some((_, lines)) = self.lines.iter().find(|(known, _)| *known == target) {
            request.request_lines(irect::point(pointer.point()), lines);
        }
    }
}

struct CreativeFrame<'a> {
    state: &'a CreativeScreenState,
    content: &'a CreativeTabContent,
    stacks: &'a MenuStacks,
    drag: &'a DragPreview,
    slot_icons: &'a Query<'a, 'a, &'static SlotIcon>,
    atlas: &'a GuiAtlasLayout,
    font: &'a Font,
    language: &'a ClientLanguage,
    now_ms: u64,
    size: GuiSize,
    pointer: GuiPointer,
}

impl CreativeFrame<'_> {
    fn slot_icon(&self, stack: Entity) -> Option<SlotIcon> {
        self.slot_icons.get(stack).ok().copied()
    }

    fn carried(&self) -> Option<SlotIcon> {
        self.stacks
            .carried()
            .and_then(|stack| self.slot_icon(stack))
    }
}

fn creative_screen(
    frame: &CreativeFrame,
    icons: &ItemIcons,
    text_tooltip: &CreativeTextTooltip,
    layer: &mut GuiLayer,
    request: &mut Tooltip,
) {
    let window = CreativeGeometry::window(frame.size.width, frame.size.height);
    let top_left = irect::point(window);
    let state = frame.state;
    let tab = state.selected_tab();
    let visible = frame.content.visible_tabs();
    let out = layer.redraw();
    out.push(GuiQuad::FillGradient {
        rect: IRect::new(0, 0, frame.size.width, frame.size.height),
        top: DIM_TOP,
        bottom: DIM_BOTTOM,
    });
    for other in unselected_tabs(visible, state.selected()) {
        out.push(local_sprite(
            top_left,
            other.tab().sprite_rect(),
            other.tab().unselected_sprite().path(),
        ));
    }
    menu_screen::background(frame.atlas, top_left, tab.kind.background().part(), out);
    if let Some(placement) = FigurePlacement::for_creative_tab(tab.kind) {
        out.push(GuiQuad::Figure {
            rect: placement.rect_at(top_left),
        });
    }
    if tab.kind == CreativeTabKind::Search {
        edit_box_draw::draw_edit_box_in(
            EditBoxFrame::new(offset(window, CreativeGeometry::SEARCH_BOX), frame.now_ms),
            state.search_box(),
            frame.atlas,
            frame.font,
            out,
        );
    }
    let len = state.content(frame.content).len();
    if let Some(thumb) = CreativeGeometry::thumb_sprite(tab.kind, len) {
        out.push(local_sprite(
            top_left,
            CreativeGeometry::thumb(state.scroll()),
            thumb.path(),
        ));
    }
    out.push(local_sprite(
        top_left,
        tab.sprite_rect(),
        tab.selected_sprite().path(),
    ));
    if tab.kind.has_title() {
        text_draw::draw_str_without_shadow(
            frame.font,
            (top_left + irect::point(CreativeGeometry::TITLE)).as_vec2(),
            frame.language.get(tab.title_key),
            LABEL_COLOR,
            out,
        );
    }
    draw_tab_icon(frame, icons, layer, top_left, state.selected());
    for other in unselected_tabs(visible, state.selected()) {
        draw_tab_icon(frame, icons, layer, top_left, other);
    }
    let hit = CreativeGeometry::hit(window, frame.pointer.point(), tab.kind, visible);
    draw_slots(frame, icons, layer, window, tab.kind, hit);
    request_tab_tooltip(frame, text_tooltip, request, window);
    request_destroy_tooltip(frame, text_tooltip, request, hit);
    let carried = frame.carried();
    menu_screen::draw_carried(
        layer,
        carried,
        frame.drag,
        irect::point(frame.pointer.point()),
    );
}

fn unselected_tabs(
    visible: VisibleTabs,
    selected: CreativeTabIndex,
) -> impl Iterator<Item = CreativeTabIndex> {
    CreativeTabIndex::all().filter(move |tab| *tab != selected && visible.contains(*tab))
}

fn draw_tab_icon(
    frame: &CreativeFrame,
    icons: &ItemIcons,
    layer: &mut GuiLayer,
    top_left: IVec2,
    tab: CreativeTabIndex,
) {
    let Some(stack) = frame.content.tab_icon(tab) else {
        return;
    };
    let at = top_left + irect::point(tab.tab().icon_position());
    icons.draw_stack(layer, at, stack, Overlays::None);
}

fn draw_slots(
    frame: &CreativeFrame,
    icons: &ItemIcons,
    layer: &mut GuiLayer,
    window: GuiPoint,
    kind: CreativeTabKind,
    hit: CreativeHit,
) {
    let top_left = irect::point(window);
    let highlight = slot_corner(hit).map(|corner| top_left + irect::point(corner));
    if let Some(at) = highlight {
        layer.push(highlight_sprite(at, "container/slot_highlight_back"));
    }
    if kind == CreativeTabKind::SurvivalInventory {
        for index in FIRST_INVENTORY_ENTRY..=LAST_INVENTORY_ENTRY {
            if let Some(corner) = CreativeGeometry::inventory_entry(index) {
                draw_entry(frame, top_left + irect::point(corner), index, layer);
            }
        }
    } else {
        for slot in 0..GRID_SLOTS {
            let corner = CreativeGeometry::grid_slot(slot);
            let stack = frame.state.grid_stack(slot, frame.content);
            if let (Some(corner), Some(stack)) = (corner, stack) {
                let at = top_left + irect::point(corner);
                icons.draw_stack(layer, at, stack, Overlays::Stack);
            }
        }
        for column in 0..HOTBAR_COLUMNS {
            if let Some(corner) = CreativeGeometry::hotbar_slot(column) {
                let at = top_left + irect::point(corner);
                draw_entry(frame, at, HOTBAR_MENU_START + column, layer);
            }
        }
    }
    if let Some(at) = highlight {
        layer.push(highlight_sprite(at, "container/slot_highlight_front"));
    }
}

pub(crate) fn slot_corner(hit: CreativeHit) -> Option<GuiPoint> {
    match hit {
        CreativeHit::GridSlot(slot) => CreativeGeometry::grid_slot(slot),
        CreativeHit::HotbarSlot(column) => CreativeGeometry::hotbar_slot(column),
        CreativeHit::InventoryEntry(index) => CreativeGeometry::inventory_entry(index),
        CreativeHit::DestroySlot => Some(CreativeGeometry::DESTROY_SLOT),
        CreativeHit::Tab(_)
        | CreativeHit::ScrollBar
        | CreativeHit::SearchBox
        | CreativeHit::Empty
        | CreativeHit::Outside => None,
    }
}

fn draw_entry(frame: &CreativeFrame, at: IVec2, index: u16, layer: &mut GuiLayer) {
    if let Some(preview) = frame.drag.entry(index)
        && frame.stacks.carried().is_some()
    {
        menu_screen::draw_previewed_entry(layer, at, preview, frame.carried());
        return;
    }
    match frame.stacks.get(index) {
        Some(stack) => {
            if let Some(slot) = frame.slot_icon(stack) {
                ItemIcons::draw_icon(layer, at, slot.icon, slot.overlay, Overlays::Stack);
            }
        }
        None => {
            if let Some(icon) = menu_screen::empty_icon(index) {
                layer.push(crate::gui_quad::sprite(at, IVec2::splat(ENTRY_SIZE), icon));
            }
        }
    }
}

fn request_tab_tooltip(
    frame: &CreativeFrame,
    text_tooltip: &CreativeTextTooltip,
    request: &mut Tooltip,
    window: GuiPoint,
) {
    let Some(tab) =
        CreativeGeometry::tab_under(window, frame.pointer.point(), frame.content.visible_tabs())
    else {
        return;
    };
    text_tooltip.request(TextTarget::Tab(tab), frame.pointer, request);
}

fn request_destroy_tooltip(
    frame: &CreativeFrame,
    text_tooltip: &CreativeTextTooltip,
    request: &mut Tooltip,
    hit: CreativeHit,
) {
    if hit != CreativeHit::DestroySlot || frame.stacks.carried().is_some() {
        return;
    }
    text_tooltip.request(TextTarget::DestroySlot, frame.pointer, request);
}

fn highlight_sprite(at: IVec2, region: &'static str) -> GuiQuad {
    crate::gui_quad::sprite(at - HIGHLIGHT_INSET, IVec2::splat(HIGHLIGHT_SIZE), region)
}

pub(crate) fn offset(window: GuiPoint, rect: GuiRect) -> GuiRect {
    GuiRect::new(
        window.x.saturating_add(rect.x),
        window.y.saturating_add(rect.y),
        rect.width,
        rect.height,
    )
}

fn local_sprite(top_left: IVec2, rect: GuiRect, region: &'static str) -> GuiQuad {
    crate::gui_quad::sprite(
        top_left + IVec2::new(rect.x, rect.y),
        IVec2::new(rect.width, rect.height),
        region,
    )
}
