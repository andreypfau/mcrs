use bevy::math::{IRect, IVec2, Rect, UVec2};
use bevy::prelude::Entity;
use mcrs_minecraft_client_widget::{GuiPoint, GuiRect};
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_inventory_click::EntryPreview;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_item_icon::{Overlays, StackOverlay};
use mcrs_minecraft_menu_geometry::{
    BackgroundPart, CreativeBackground, MenuGeometry, MenuLabels, TitlePosition,
};
use mcrs_minecraft_menu_key::menu_keys::MenuType;
use mcrs_minecraft_resource::ResourceLocation;

use crate::drag_preview::DragPreview;
use crate::font::{Font, draw_text_without_shadow};
use crate::inventory_figure::figure_rect;
use crate::item_decorations::WHITE;
use crate::item_icons::ItemIcons;
use crate::menu_draw;
use crate::menu_label_text::MenuLabelText;
use crate::menu_stacks::MenuStacks;
use crate::scene::{GuiAtlasData, GuiQuad, GuiScene, GuiSize, sprite};
use crate::slot_icon::SlotIcon;

const RECIPE_BOOK_BUTTON: GuiRect = GuiRect::new(104, 61, 20, 18);
const DRAG_FILL: u32 = 0x80FF_FFFF;
pub(crate) const DIM_TOP: u32 = 0xC010_1010;
pub(crate) const DIM_BOTTOM: u32 = 0xD010_1010;
const TEXTURE_FOLDER: &str = "textures/gui/";
const TEXTURE_SUFFIX: &str = ".png";

pub(crate) fn empty_icon(menu: u16) -> Option<&'static str> {
    match menu {
        slots::ARMOR_HEAD => Some("container/slot/helmet"),
        slots::ARMOR_CHEST => Some("container/slot/chestplate"),
        slots::ARMOR_LEGS => Some("container/slot/leggings"),
        slots::ARMOR_FEET => Some("container/slot/boots"),
        slots::OFFHAND => Some("container/slot/shield"),
        _ => None,
    }
}

pub(crate) fn inventory_window_corner(size: GuiSize) -> IVec2 {
    menu_draw::point(MenuGeometry::INVENTORY.window(size.width, size.height))
}

pub(crate) fn recipe_book_button(corner: GuiPoint) -> GuiRect {
    GuiRect::new(
        corner.x.saturating_add(RECIPE_BOOK_BUTTON.x),
        corner.y.saturating_add(RECIPE_BOOK_BUTTON.y),
        RECIPE_BOOK_BUTTON.width,
        RECIPE_BOOK_BUTTON.height,
    )
}

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

#[derive(Clone, Copy, Debug)]
pub struct MenuContents<'a> {
    pub stacks: &'a MenuStacks,
    pub drag: &'a DragPreview,
}

pub fn menu_screen(
    scene: &mut GuiScene,
    geometry: MenuGeometry,
    contents: MenuContents,
    labels: &MenuLabelText,
    atlas: &GuiAtlasData,
    icon_of: impl Fn(Entity) -> Option<SlotIcon>,
) {
    let MenuContents { stacks, drag } = contents;
    let (size, cursor) = (scene.size, scene.cursor);
    let corner = geometry.window(size.width, size.height);
    let top_left = menu_draw::point(corner);
    let pointer = GuiPoint::new(cursor.x, cursor.y);
    let inventory = geometry.is_inventory();
    scene.quads.push(GuiQuad::FillGradient {
        rect: IRect::new(0, 0, size.width, size.height),
        top: DIM_TOP,
        bottom: DIM_BOTTOM,
    });
    for part in geometry.background() {
        background(atlas, top_left, part, &mut scene.quads);
    }
    if inventory {
        inventory_extras(&mut scene.quads, corner, pointer);
    }
    if let Some(positions) = geometry.labels() {
        draw_labels(
            &atlas.font,
            top_left,
            &geometry,
            positions,
            labels,
            &mut scene.quads,
        );
    }
    let highlight = match geometry.clicked_at(corner, pointer) {
        Clicked::Entry(index) => geometry.entry(index).map(menu_draw::point),
        Clicked::Outside | Clicked::Nowhere => None,
    };
    if let Some(position) = highlight {
        scene.quads.push(sprite(
            top_left + position - 4,
            IVec2::new(24, 24),
            "container/slot_highlight_back",
        ));
    }
    let carried_stack = stacks.carried();
    let carried = carried_stack.and_then(&icon_of);
    for entry in geometry.entries() {
        let at = top_left + menu_draw::point(entry.corner);
        if let Some(preview) = drag.entry(entry.index)
            && carried_stack.is_some()
        {
            draw_previewed_entry(scene, at, preview, carried);
            continue;
        }
        match stacks.get(entry.index) {
            Some(stack) => {
                if let Some(slot) = icon_of(stack) {
                    ItemIcons::draw_icon(scene, at, slot.icon, slot.overlay, Overlays::Stack);
                }
            }
            None => {
                if let Some(icon) = empty_icon(entry.index).filter(|_| inventory) {
                    scene.quads.push(sprite(at, IVec2::splat(16), icon));
                }
            }
        }
    }
    if let Some(position) = highlight {
        scene.quads.push(sprite(
            top_left + position - 4,
            IVec2::new(24, 24),
            "container/slot_highlight_front",
        ));
    }
    draw_carried(scene, carried, drag);
}

pub(crate) fn draw_previewed_entry(
    scene: &mut GuiScene,
    at: IVec2,
    preview: EntryPreview,
    carried: Option<SlotIcon>,
) {
    scene.quads.push(GuiQuad::Fill {
        rect: IRect::from_corners(at, at + 16),
        color: DRAG_FILL,
    });
    if let Some(slot) = carried {
        let (count, overlays) = match preview {
            EntryPreview::Fits(count) => (count, Overlays::Stack),
            EntryPreview::Capped(count) => (count, Overlays::Capped),
        };
        let overlay = StackOverlay {
            count,
            ..slot.overlay
        };
        ItemIcons::draw_icon(scene, at, slot.icon, overlay, overlays);
    }
}

pub(crate) fn draw_carried(scene: &mut GuiScene, carried: Option<SlotIcon>, drag: &DragPreview) {
    if let Some(slot) = carried {
        let count = drag.cursor().unwrap_or(slot.overlay.count);
        if count != 0 {
            let overlay = StackOverlay {
                count,
                ..slot.overlay
            };
            let at = scene.cursor - 8;
            ItemIcons::draw_icon(scene, at, slot.icon, overlay, Overlays::Stack);
        }
    }
}

fn inventory_extras(out: &mut Vec<GuiQuad>, corner: GuiPoint, pointer: GuiPoint) {
    let top_left = menu_draw::point(corner);
    out.push(GuiQuad::Figure {
        rect: figure_rect(top_left),
    });
    let button = recipe_book_button(corner);
    let button_area = menu_draw::irect(button);
    out.push(sprite(
        button_area.min,
        button_area.size(),
        if button.contains_point(pointer) {
            "recipe_book/button_highlighted"
        } else {
            "recipe_book/button"
        },
    ));
}

pub(crate) fn background(
    atlas: &GuiAtlasData,
    top_left: IVec2,
    part: BackgroundPart,
    out: &mut Vec<GuiQuad>,
) {
    let Some(region) = background_region(part.texture).and_then(|name| atlas.regions.get(name))
    else {
        return;
    };
    let source = IRect::new(
        part.source.x,
        part.source.y,
        part.source.x.saturating_add(part.source.width),
        part.source.y.saturating_add(part.source.height),
    );
    let at = top_left + menu_draw::point(part.destination);
    out.push(GuiQuad::Texels {
        rect: IRect::from_corners(at, at + source.size()),
        texels: Rect::from_corners(
            (region.min + source.min).as_vec2(),
            (region.min + source.max).as_vec2(),
        ),
        color: WHITE,
    });
}

fn draw_labels(
    font: &Font,
    top_left: IVec2,
    geometry: &MenuGeometry,
    positions: MenuLabels,
    labels: &MenuLabelText,
    out: &mut Vec<GuiQuad>,
) {
    let title = match positions.title {
        TitlePosition::Left(at) => Some(menu_draw::point(at)),
        TitlePosition::Centred { y } => Some(IVec2::new(
            geometry
                .width()
                .saturating_sub(font.width(labels.title()))
                .div_euclid(2),
            y,
        )),
        _ => None,
    };
    if let Some(title) = title {
        draw_text_without_shadow(font, top_left + title, labels.title(), out);
    }
    draw_text_without_shadow(
        font,
        top_left + menu_draw::point(positions.player_inventory),
        labels.inventory(),
        out,
    );
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use bevy::app::App;
    use bevy::math::Vec2;
    use mcrs_minecraft_engine_inventory_menu::MenuParts;
    use mcrs_minecraft_engine_inventory_menu::MenuTitle;
    use mcrs_minecraft_engine_inventory_menu::Menus;
    use mcrs_minecraft_engine_item::ItemCommandsExt;
    use mcrs_minecraft_engine_item::ItemPlugin;
    use mcrs_minecraft_engine_item::Slots;
    use mcrs_minecraft_engine_item::StackCommandsExt;
    use mcrs_minecraft_inventory_click::DragEntry;
    use mcrs_minecraft_inventory_click::DragKind;
    use mcrs_minecraft_inventory_click::DragSplit;
    use mcrs_minecraft_inventory_file::PlayerItems;
    use mcrs_minecraft_inventory_menu::MenuLayout;
    use mcrs_minecraft_item_icon::IconSources;
    use mcrs_minecraft_item_stack::Builtin;
    use mcrs_minecraft_item_stack::Count;
    use mcrs_minecraft_item_stack::ItemStack;
    use mcrs_minecraft_item_stack::Kind;
    use mcrs_minecraft_item_stack::MaxStackSize;
    use mcrs_minecraft_item_stack::NameLookup;
    use mcrs_minecraft_item_stack::NameTable;
    use mcrs_minecraft_item_table::ItemTable;
    use mcrs_minecraft_menu_key::menu_keys::MenuType;
    use mcrs_minecraft_model::resource_set::ResourceSet;
    use mcrs_minecraft_model::stack_view::StackView;
    use mcrs_minecraft_player_holders::PlayerHolders;
    use mcrs_minecraft_player_holders::PlayerHoldersPlugin;
    use mcrs_minecraft_player_holders::restore_holders;
    use mcrs_minecraft_text::Text;

    use super::*;
    use crate::hotbar::hotbar;
    use crate::language::ClientLanguage;
    use crate::menu_label_text::LABEL_COLOR;
    use crate::menu_test_app::{MODELS, TABLE};

    const SIZE: GuiSize = GuiSize {
        width: 427,
        height: 240,
        scale: 2,
    };

    static ATLAS: LazyLock<GuiAtlasData> =
        LazyLock::new(|| GuiAtlasData::load(ResourceSet::corpus()).unwrap());

    static ICON: LazyLock<SlotIcon> = LazyLock::new(|| {
        let location = ResourceLocation::read("minecraft:stone").unwrap();
        let kind = Kind(TABLE.id(NameTable::Item, location).unwrap());
        let stack = ItemStack::new(kind, Count::new(1).unwrap());
        let view = StackView {
            stack: &stack,
            table: &TABLE,
        };
        let sources = IconSources {
            models: &MODELS,
            registries: None,
        };
        SlotIcon::of(view, &sources)
    });

    fn scene(cursor: IVec2) -> GuiScene {
        GuiScene {
            size: SIZE,
            cursor,
            ..GuiScene::default()
        }
    }

    enum Place {
        Cursor,
        CraftResult,
        CraftGrid(u16),
        Player(u16),
        Container(u16),
    }

    fn stacks(container: Option<MenuType>, filled: &[Place]) -> MenuStacks {
        let mut app = App::new();
        app.insert_resource(ItemTable::leak([vec![Builtin::MaxStackSize(
            MaxStackSize(64),
        )]]))
        .add_plugins((ItemPlugin::default(), PlayerHoldersPlugin));
        let world = app.world_mut();
        let player = world.spawn_empty().id();
        world
            .run_system_cached_with(restore_holders, (player, PlayerItems::default()))
            .unwrap();
        world.flush();
        let chest = world.spawn(Slots::new(27)).id();
        let holders = *world.get::<PlayerHolders>(player).unwrap();
        let (layout, parts) = match container {
            None => {
                let menu = world.get::<Menus>(player).unwrap().iter().next().unwrap();
                (
                    world.get::<MenuLayout>(menu).unwrap().clone(),
                    *world.get::<MenuParts>(menu).unwrap(),
                )
            }
            Some(kind) => (
                MenuLayout::of(kind).unwrap(),
                MenuParts::new([chest, player]).unwrap(),
            ),
        };
        for place in filled {
            let (holder, index) = match place {
                Place::Cursor => (holders.cursor, 0),
                Place::CraftResult => (holders.craft_result, 0),
                Place::CraftGrid(index) => (holders.craft_grid, *index),
                Place::Player(index) => (player, *index),
                Place::Container(index) => (chest, *index),
            };
            let stack = ItemStack::new(Kind(0), Count::new(1).unwrap());
            let world = app.world_mut();
            world.commands().spawn_stack(stack).put_at(holder, index);
            world.flush();
        }
        let mut stacks = MenuStacks::default();
        stacks.collect(&layout, &parts, holders.cursor, |slot| {
            app.world()
                .get::<Slots>(slot.holder)?
                .get(usize::from(slot.index))
        });
        stacks
    }

    fn filled(stacks: &MenuStacks) -> Vec<Option<u16>> {
        let entries = (0..100).filter(|&index| stacks.get(index).is_some());
        entries
            .map(Some)
            .chain(stacks.carried().map(|_| None))
            .collect()
    }

    #[test]
    fn the_menu_stacks_follow_the_vanilla_menu_layout() {
        let chest = Some(MenuType::Generic9x3);
        let placed = [
            (None, Place::CraftResult, Some(0)),
            (None, Place::CraftGrid(0), Some(1)),
            (None, Place::CraftGrid(3), Some(4)),
            (None, Place::Player(39), Some(5)),
            (None, Place::Player(36), Some(8)),
            (None, Place::Player(9), Some(9)),
            (None, Place::Player(35), Some(35)),
            (None, Place::Player(0), Some(36)),
            (None, Place::Player(8), Some(44)),
            (None, Place::Player(40), Some(45)),
            (None, Place::Cursor, None),
            (chest, Place::Container(26), Some(26)),
            (chest, Place::Player(9), Some(27)),
            (chest, Place::Player(0), Some(54)),
            (chest, Place::Cursor, None),
        ];
        for (container, place, menu) in placed {
            assert_eq!(filled(&stacks(container, &[place])), vec![menu]);
        }
    }

    #[test]
    fn the_hotbar_lists_the_vanilla_rectangles_in_order() {
        let table = stacks(None, &[Place::Player(3), Place::Player(40)]);
        let mut scene = scene(IVec2::ZERO);
        hotbar(&mut scene, 3, &table, |_| Some(*ICON));
        let out = &scene.quads;
        let cx = 213;
        assert_eq!(
            out[0],
            GuiQuad::Sprite {
                rect: IRect::new(cx - 91, 218, cx + 91, 240),
                region: "hud/hotbar",
                color: WHITE
            }
        );
        assert_eq!(
            out[1],
            GuiQuad::Sprite {
                rect: IRect::new(cx - 92 + 60, 217, cx - 92 + 60 + 24, 240),
                region: "hud/hotbar_selection",
                color: WHITE
            }
        );
        assert!(
            matches!(out[2], GuiQuad::Sprite { rect, region: "hud/hotbar_offhand_left", .. } if rect.min == IVec2::new(cx - 120, 217))
        );
        assert!(
            matches!(out[3], GuiQuad::Icon { origin, .. } if origin == IVec2::new(cx - 90 + 62, 221))
        );
        assert!(
            matches!(out[4], GuiQuad::Icon { origin, .. } if origin == IVec2::new(cx - 117, 221))
        );
        assert_eq!(out.len(), 5);
    }

    fn items(out: &[GuiQuad]) -> Vec<IVec2> {
        out.iter()
            .filter_map(|quad| match quad {
                GuiQuad::Icon { origin, .. } => Some(*origin),
                _ => None,
            })
            .collect()
    }

    fn texels(out: &[GuiQuad]) -> Vec<(IRect, Rect)> {
        out.iter()
            .filter_map(|quad| match quad {
                GuiQuad::Texels { rect, texels, .. } => Some((*rect, *texels)),
                _ => None,
            })
            .collect()
    }

    fn label_origins(out: &[GuiQuad]) -> Vec<IVec2> {
        let mut origins: Vec<IVec2> = Vec::new();
        for quad in out {
            if let GuiQuad::Glyph { pen, color, .. } = quad {
                assert_eq!(*color, LABEL_COLOR, "the labels draw in dark grey");
                let pen = pen.as_ivec2();
                if origins.last().is_none_or(|last| last.y != pen.y) {
                    origins.push(pen);
                }
            }
        }
        origins
    }

    #[test]
    fn the_inventory_screen_lists_the_vanilla_rectangles_in_order() {
        let table = stacks(None, &[Place::Player(9), Place::Cursor]);
        let cursor = IVec2::new(125 + 8 + 5, 37 + 84 + 5);
        let mut scene = scene(cursor);
        menu_screen(
            &mut scene,
            MenuGeometry::INVENTORY,
            MenuContents {
                stacks: &table,
                drag: &DragPreview::default(),
            },
            &MenuLabelText::default(),
            &ATLAS,
            |_| Some(*ICON),
        );
        let out = &scene.quads;
        let top_left = IVec2::new(125, 37);
        assert_eq!(inventory_window_corner(SIZE), top_left);
        assert!(
            matches!(out[0], GuiQuad::FillGradient { rect, top: 0xC010_1010, bottom: 0xD010_1010 } if rect == IRect::new(0, 0, 427, 240)),
            "the dim gradient covers the whole screen first"
        );
        let image = ATLAS.region("container/inventory").min.as_vec2();
        assert_eq!(
            out.get(1),
            Some(&GuiQuad::Texels {
                rect: IRect::from_corners(top_left, top_left + IVec2::new(176, 166)),
                texels: Rect::from_corners(image, image + Vec2::new(176.0, 166.0)),
                color: WHITE,
            })
        );
        assert_eq!(
            out.get(2),
            Some(&GuiQuad::Figure {
                rect: IRect::new(151, 45, 200, 115)
            }),
            "the figure draws over the image and under the slots and the carried stack"
        );
        assert!(
            matches!(out[3], GuiQuad::Sprite { rect, region: "recipe_book/button", .. } if rect.min == IVec2::new(229, 98)),
            "the recipe book button draws over the figure"
        );
        assert!(
            matches!(out[4], GuiQuad::Sprite { rect, region: "container/slot_highlight_back", .. } if rect.min == top_left + IVec2::new(4, 80)),
            "the back highlight of the slot under the cursor draws under the stacks"
        );
        let armour: Vec<_> = out
            .iter()
            .filter_map(|q| match q {
                GuiQuad::Sprite { region, rect, .. } if region.starts_with("container/slot/") => {
                    Some((*region, rect.min))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            armour,
            vec![
                ("container/slot/helmet", top_left + IVec2::new(8, 8)),
                ("container/slot/chestplate", top_left + IVec2::new(8, 26)),
                ("container/slot/leggings", top_left + IVec2::new(8, 44)),
                ("container/slot/boots", top_left + IVec2::new(8, 62)),
                ("container/slot/shield", top_left + IVec2::new(77, 62)),
            ]
        );
        assert_eq!(items(out), vec![top_left + IVec2::new(8, 84), cursor - 8]);
        assert!(label_origins(out).is_empty(), "the inventory has no labels");
        assert!(
            matches!(
                out[out.len() - 2],
                GuiQuad::Sprite {
                    region: "container/slot_highlight_front",
                    ..
                }
            ),
            "the front highlight draws over the stacks"
        );
        assert!(
            matches!(out[out.len() - 1], GuiQuad::Icon { .. }),
            "the carried stack draws last"
        );
    }

    #[test]
    fn a_drag_of_ten_over_three_empty_entries_previews_three_and_caps_the_small_one() {
        let table = stacks(None, &[Place::Cursor]);
        let carried = table.carried().unwrap();
        let mut drag = DragPreview::default();
        for (index, cap) in [(9, 64), (10, 64), (11, 2)] {
            drag.join(index, DragEntry { held: 0, cap });
        }
        drag.split(DragSplit {
            kind: DragKind::Split,
            count: 10,
            max: 64,
            size: 3,
        });
        let cursor = IVec2::new(300, 200);
        let mut scene = scene(cursor);
        menu_screen(
            &mut scene,
            MenuGeometry::INVENTORY,
            MenuContents {
                stacks: &table,
                drag: &drag,
            },
            &MenuLabelText::default(),
            &ATLAS,
            |stack| (stack == carried).then_some(*ICON),
        );
        let out = &scene.quads;
        let top_left = inventory_window_corner(SIZE);
        let corners: Vec<IVec2> = (9..12)
            .map(|index| top_left + menu_draw::point(MenuGeometry::INVENTORY.entry(index).unwrap()))
            .collect();
        let fills: Vec<IRect> = (out.iter())
            .filter_map(|quad| match quad {
                GuiQuad::Fill {
                    rect,
                    color: 0x80FF_FFFF,
                } => Some(*rect),
                _ => None,
            })
            .collect();
        let entry_areas: Vec<IRect> = (corners.iter())
            .map(|corner| IRect::from_corners(*corner, *corner + 16))
            .collect();
        assert_eq!(fills, entry_areas, "a white fill on each previewed entry");
        let drawn: Vec<(IVec2, u8, Overlays)> = (out.iter())
            .filter_map(|quad| match *quad {
                GuiQuad::Icon { origin, index } => {
                    let entry = scene.icons[index];
                    Some((origin, entry.overlay.count, entry.overlays))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            drawn,
            vec![
                (corners[0], 3, Overlays::Stack),
                (corners[1], 3, Overlays::Stack),
                (corners[2], 2, Overlays::Capped),
                (cursor - 8, 2, Overlays::Stack),
            ],
            "3 in each entry that fits, 2 capped in the small one and 2 at the mouse"
        );
    }

    #[test]
    fn a_chest_screen_draws_the_background_labels_and_stacks_of_its_menu_type() {
        let table = stacks(
            Some(MenuType::Generic9x3),
            &[Place::Container(0), Place::Player(0), Place::Cursor],
        );
        let mut labels = MenuLabelText::default();
        labels.write(
            Some(&MenuTitle(Text::text("Chest"))),
            ClientLanguage::corpus(),
        );
        let geometry = MenuGeometry::of(MenuType::Generic9x3).unwrap();
        let top_left = IVec2::new((427 - 176) / 2, (240 - 168) / 2);
        let cursor = top_left + IVec2::new(8 + 3, 18 + 3);
        let mut scene = scene(cursor);
        menu_screen(
            &mut scene,
            geometry,
            MenuContents {
                stacks: &table,
                drag: &DragPreview::default(),
            },
            &labels,
            &ATLAS,
            |_| Some(*ICON),
        );
        let out = &scene.quads;

        let image = ATLAS.region("container/generic_54").min.as_vec2();
        assert_eq!(
            texels(out),
            vec![
                (
                    IRect::from_corners(top_left, top_left + IVec2::new(176, 71)),
                    Rect::from_corners(image, image + Vec2::new(176.0, 71.0)),
                ),
                (
                    IRect::from_corners(
                        top_left + IVec2::new(0, 71),
                        top_left + IVec2::new(176, 167)
                    ),
                    Rect::from_corners(
                        image + Vec2::new(0.0, 126.0),
                        image + Vec2::new(176.0, 222.0)
                    ),
                ),
            ],
            "the two parts of generic_54 touch"
        );
        assert_eq!(
            label_origins(out),
            vec![top_left + IVec2::new(8, 6), top_left + IVec2::new(8, 74)],
            "the title and the inventory label"
        );
        assert_eq!(
            items(out),
            vec![
                top_left + IVec2::new(8, 18),
                top_left + IVec2::new(8, 143),
                cursor - 8
            ],
            "the container slot 0, P[0], then the carried stack"
        );
        assert!(
            out.contains(&sprite(
                top_left + IVec2::new(4, 14),
                IVec2::new(24, 24),
                "container/slot_highlight_back"
            )),
            "the back highlight marks the container slot under the cursor"
        );
        assert!(
            !out.iter().any(|quad| matches!(
                quad,
                GuiQuad::Figure { .. }
                    | GuiQuad::Sprite {
                        region: "recipe_book/button",
                        ..
                    }
            )),
            "the figure and the recipe book button are parts of the inventory screen only"
        );
    }
}
