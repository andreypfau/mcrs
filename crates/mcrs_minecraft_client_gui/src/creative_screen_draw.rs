use bevy::math::{IRect, IVec2};
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_client_widget::{GuiPoint, GuiRect};
use mcrs_minecraft_client_world::player::Player;
use mcrs_minecraft_engine_inventory_menu::{MenuParts, Menus};
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_icon::Overlays;
use mcrs_minecraft_menu_geometry::{
    CREATIVE_TABS, CreativeGeometry, CreativeHit, CreativeTabIndex, CreativeTabKind, VisibleTabs,
};
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;
use crate::edit_box_draw::draw_edit_box_in;
use crate::font::{Span, draw_str_without_shadow};
use crate::item_decorations::WHITE;
use crate::item_icons::ItemIcons;
use crate::language::ClientLanguage;
use crate::menu_draw;
use crate::menu_input::menu_clock;
use crate::menu_label_text::LABEL_COLOR;
use crate::menu_screen::{DIM_BOTTOM, DIM_TOP, background, empty_icon};
use crate::menu_stacks::MenuStacks;
use crate::scene::{GuiAtlas, GuiAtlasData, GuiQuad, GuiScene, sprite};
use crate::slot_icon::SlotIcon;

const GRID_SLOTS: u16 = 45;
const HOTBAR_COLUMNS: u16 = 9;
const HOTBAR_MENU_START: u16 = 36;
const FIRST_INVENTORY_ENTRY: u16 = 5;
const LAST_INVENTORY_ENTRY: u16 = 45;
pub(crate) const ENTRY_SIZE: i32 = 16;
const HIGHLIGHT_INSET: i32 = 4;
const HIGHLIGHT_SIZE: i32 = 24;
const CARRIED_OFFSET: i32 = 8;
const DESTROY_SLOT_KEY: &str = "inventory.binSlot";

pub(crate) fn creative_sprites() -> impl Iterator<Item = &'static str> {
    CREATIVE_TABS
        .into_iter()
        .flat_map(|tab| [tab.selected_sprite(), tab.unselected_sprite()])
        .chain([
            CreativeGeometry::SCROLLER,
            CreativeGeometry::SCROLLER_DISABLED,
        ])
        .map(|location| location.path())
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each access is its own parameter"
)]
pub fn draw_creative_screen(
    player: Query<(&PlayerHolders, &Menus), With<Player>>,
    menus: Query<(&MenuLayout, &MenuParts)>,
    holders: Query<&Slots>,
    creative: Res<CreativeScreenState>,
    content: Res<CreativeTabContent>,
    atlas: Option<Res<GuiAtlas>>,
    language: Option<Res<ClientLanguage>>,
    time: Res<Time<Real>>,
    icons: ItemIcons,
    slot_icons: Query<&'static SlotIcon>,
    mut stacks: Local<MenuStacks>,
    mut scene: ResMut<GuiScene>,
) {
    let (Some(atlas), Some(language)) = (atlas, language) else {
        return;
    };
    let Ok((player, player_menus)) = player.single() else {
        return;
    };
    let Some((layout, parts)) = player_menus.open().and_then(|menu| menus.get(menu).ok()) else {
        return;
    };
    stacks.collect(layout, parts, player.cursor, |slot| {
        holders.get(slot.holder).ok()?.get(usize::from(slot.index))
    });
    let frame = CreativeFrame {
        state: &creative,
        content: &content,
        stacks: &stacks,
        slot_icons: &slot_icons,
        atlas: &atlas.0,
        language: &language,
        now_ms: menu_clock(&time),
    };
    creative_screen(&frame, &icons, &mut scene);
}

struct CreativeFrame<'a> {
    state: &'a CreativeScreenState,
    content: &'a CreativeTabContent,
    stacks: &'a MenuStacks,
    slot_icons: &'a Query<'a, 'a, &'static SlotIcon>,
    atlas: &'a GuiAtlasData,
    language: &'a ClientLanguage,
    now_ms: u64,
}

impl CreativeFrame<'_> {
    fn slot_icon(&self, stack: Entity) -> Option<SlotIcon> {
        self.slot_icons.get(stack).ok().copied()
    }
}

fn creative_screen(frame: &CreativeFrame, icons: &ItemIcons, scene: &mut GuiScene) {
    let window = CreativeGeometry::window(scene.size.width, scene.size.height);
    let top_left = menu_draw::point(window);
    let state = frame.state;
    let tab = state.selected_tab();
    let visible = frame.content.visible_tabs();
    scene.quads.push(GuiQuad::FillGradient {
        rect: IRect::new(0, 0, scene.size.width, scene.size.height),
        top: DIM_TOP,
        bottom: DIM_BOTTOM,
    });
    for other in unselected_tabs(visible, state.selected()) {
        scene.quads.push(local_sprite(
            top_left,
            other.tab().sprite_rect(),
            other.tab().unselected_sprite().path(),
        ));
    }
    background(
        frame.atlas,
        top_left,
        tab.kind.background().part(),
        &mut scene.quads,
    );
    if tab.kind == CreativeTabKind::Search {
        draw_edit_box_in(
            offset(window, CreativeGeometry::SEARCH_BOX),
            None,
            state.search_box(),
            frame.now_ms,
            &frame.atlas.font,
            &mut scene.quads,
        );
    }
    let len = state.content(frame.content).len();
    if let Some(thumb) = CreativeGeometry::thumb_sprite(tab.kind, len) {
        scene.quads.push(local_sprite(
            top_left,
            CreativeGeometry::thumb(state.scroll()),
            thumb.path(),
        ));
    }
    scene.quads.push(local_sprite(
        top_left,
        tab.sprite_rect(),
        tab.selected_sprite().path(),
    ));
    draw_tab_icon(frame, icons, scene, top_left, state.selected());
    for other in unselected_tabs(visible, state.selected()) {
        draw_tab_icon(frame, icons, scene, top_left, other);
    }
    if tab.kind.has_title() {
        draw_str_without_shadow(
            &frame.atlas.font,
            (top_left + menu_draw::point(CreativeGeometry::TITLE)).as_vec2(),
            frame.language.get(tab.title_key),
            LABEL_COLOR,
            &mut scene.quads,
        );
    }
    draw_slots(frame, icons, scene, window, tab.kind);
    request_tab_tooltip(frame, scene, window);
    request_destroy_tooltip(frame, scene, window, tab.kind);
    if let Some(slot) = frame
        .stacks
        .carried()
        .and_then(|stack| frame.slot_icon(stack))
    {
        let at = scene.cursor - CARRIED_OFFSET;
        ItemIcons::draw_icon(scene, at, slot.icon, slot.overlay, Overlays::Stack);
    }
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
    scene: &mut GuiScene,
    top_left: IVec2,
    tab: CreativeTabIndex,
) {
    let Some(stack) = frame.content.tab_icon(tab) else {
        return;
    };
    let at = top_left + menu_draw::point(tab.tab().icon_position());
    icons.draw_stack(scene, at, stack, Overlays::None);
}

fn draw_slots(
    frame: &CreativeFrame,
    icons: &ItemIcons,
    scene: &mut GuiScene,
    window: GuiPoint,
    kind: CreativeTabKind,
) {
    let top_left = menu_draw::point(window);
    let pointer = GuiPoint::new(scene.cursor.x, scene.cursor.y);
    let hit = CreativeGeometry::hit(window, pointer, kind, frame.content.visible_tabs());
    let highlight = slot_corner(hit).map(|corner| top_left + menu_draw::point(corner));
    if let Some(at) = highlight {
        scene
            .quads
            .push(highlight_sprite(at, "container/slot_highlight_back"));
    }
    if kind == CreativeTabKind::SurvivalInventory {
        for index in FIRST_INVENTORY_ENTRY..=LAST_INVENTORY_ENTRY {
            if let Some(corner) = CreativeGeometry::inventory_entry(index) {
                draw_entry(frame, top_left + menu_draw::point(corner), index, scene);
            }
        }
    } else {
        for slot in 0..GRID_SLOTS {
            let corner = CreativeGeometry::grid_slot(slot);
            let stack = frame.state.grid_stack(slot, frame.content);
            if let (Some(corner), Some(stack)) = (corner, stack) {
                let at = top_left + menu_draw::point(corner);
                icons.draw_stack(scene, at, stack, Overlays::Stack);
            }
        }
        for column in 0..HOTBAR_COLUMNS {
            if let Some(corner) = CreativeGeometry::hotbar_slot(column) {
                let at = top_left + menu_draw::point(corner);
                draw_entry(frame, at, HOTBAR_MENU_START + column, scene);
            }
        }
    }
    if let Some(at) = highlight {
        scene
            .quads
            .push(highlight_sprite(at, "container/slot_highlight_front"));
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

fn draw_entry(frame: &CreativeFrame, at: IVec2, index: u16, scene: &mut GuiScene) {
    match frame.stacks.get(index) {
        Some(stack) => {
            if let Some(slot) = frame.slot_icon(stack) {
                ItemIcons::draw_icon(scene, at, slot.icon, slot.overlay, Overlays::Stack);
            }
        }
        None => {
            if let Some(icon) = empty_icon(index) {
                scene.quads.push(sprite(at, IVec2::splat(ENTRY_SIZE), icon));
            }
        }
    }
}

fn request_tab_tooltip(frame: &CreativeFrame, scene: &mut GuiScene, window: GuiPoint) {
    let pointer = GuiPoint::new(scene.cursor.x, scene.cursor.y);
    let Some(tab) = CreativeGeometry::tab_under(window, pointer, frame.content.visible_tabs())
    else {
        return;
    };
    let title = frame.language.get(tab.tab().title_key);
    scene.tooltip.request(scene.cursor, |lines| {
        lines.push(vec![Span::new(title, WHITE)]);
    });
}

fn request_destroy_tooltip(
    frame: &CreativeFrame,
    scene: &mut GuiScene,
    window: GuiPoint,
    kind: CreativeTabKind,
) {
    let pointer = GuiPoint::new(scene.cursor.x, scene.cursor.y);
    let hit = CreativeGeometry::hit(window, pointer, kind, frame.content.visible_tabs());
    if hit != CreativeHit::DestroySlot || frame.stacks.carried().is_some() {
        return;
    }
    let text = frame.language.get(DESTROY_SLOT_KEY);
    scene.tooltip.request(scene.cursor, |lines| {
        lines.push(vec![Span::new(text, WHITE)]);
    });
}

fn highlight_sprite(at: IVec2, region: &'static str) -> GuiQuad {
    sprite(at - HIGHLIGHT_INSET, IVec2::splat(HIGHLIGHT_SIZE), region)
}

fn offset(window: GuiPoint, rect: GuiRect) -> GuiRect {
    GuiRect::new(
        window.x.saturating_add(rect.x),
        window.y.saturating_add(rect.y),
        rect.width,
        rect.height,
    )
}

fn local_sprite(top_left: IVec2, rect: GuiRect, region: &'static str) -> GuiQuad {
    sprite(
        top_left + IVec2::new(rect.x, rect.y),
        IVec2::new(rect.width, rect.height),
        region,
    )
}
