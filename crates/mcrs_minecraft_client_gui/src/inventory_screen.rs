use bevy::math::{IRect, IVec2};
use mcrs_minecraft_client_widget::GuiPoint;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_menu_geometry::MenuGeometry;

use crate::inventory_figure::figure_rect;
use crate::player_stacks::PlayerStacks;
use crate::scene::{GuiQuad, GuiSize, sprite};

const GEOMETRY: MenuGeometry = MenuGeometry::INVENTORY;
const DIM_TOP: u32 = 0xC010_1010;
const DIM_BOTTOM: u32 = 0xD010_1010;

fn empty_icon(menu: u16) -> Option<&'static str> {
    match menu {
        slots::ARMOR_HEAD => Some("container/slot/helmet"),
        slots::ARMOR_CHEST => Some("container/slot/chestplate"),
        slots::ARMOR_LEGS => Some("container/slot/leggings"),
        slots::ARMOR_FEET => Some("container/slot/boots"),
        slots::OFFHAND => Some("container/slot/shield"),
        _ => None,
    }
}

fn vector(point: GuiPoint) -> IVec2 {
    IVec2::new(point.x, point.y)
}

pub(crate) fn window_corner(size: GuiSize) -> IVec2 {
    vector(GEOMETRY.window(size.width, size.height))
}

pub fn inventory_screen(
    size: GuiSize,
    cursor: IVec2,
    stacks: &PlayerStacks,
    out: &mut Vec<GuiQuad>,
) {
    let corner = GEOMETRY.window(size.width, size.height);
    let top_left = vector(corner);
    out.push(GuiQuad::FillGradient {
        rect: IRect::new(0, 0, size.width, size.height),
        top: DIM_TOP,
        bottom: DIM_BOTTOM,
    });
    out.push(sprite(
        top_left,
        IVec2::new(GEOMETRY.width(), GEOMETRY.height()),
        "container/inventory",
    ));
    out.push(GuiQuad::Figure {
        rect: figure_rect(top_left),
    });
    let button = IVec2::new(top_left.x + 104, size.height / 2 - 22);
    let over_button = (button.x..button.x + 20).contains(&cursor.x)
        && (button.y..button.y + 18).contains(&cursor.y);
    out.push(sprite(
        button,
        IVec2::new(20, 18),
        if over_button {
            "recipe_book/button_highlighted"
        } else {
            "recipe_book/button"
        },
    ));
    let highlight = match GEOMETRY.clicked_at(corner, GuiPoint::new(cursor.x, cursor.y)) {
        Clicked::Entry(index) => GEOMETRY.entry(index).map(vector),
        Clicked::Outside | Clicked::Nowhere => None,
    };
    if let Some(position) = highlight {
        out.push(sprite(
            top_left + position - 4,
            IVec2::new(24, 24),
            "container/slot_highlight_back",
        ));
    }
    for entry in GEOMETRY.entries() {
        let at = top_left + vector(entry.corner);
        match stacks.get(entry.index) {
            Some(stack) => out.push(GuiQuad::Item { origin: at, stack }),
            None => {
                if let Some(icon) = empty_icon(entry.index) {
                    out.push(sprite(at, IVec2::splat(16), icon));
                }
            }
        }
    }
    if let Some(position) = highlight {
        out.push(sprite(
            top_left + position - 4,
            IVec2::new(24, 24),
            "container/slot_highlight_front",
        ));
    }
    if let Some(stack) = stacks.get(slots::CARRIED) {
        out.push(GuiQuad::Item {
            origin: cursor - 8,
            stack,
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::app::App;
    use mcrs_minecraft_engine_inventory_menu::MenuParts;
    use mcrs_minecraft_engine_inventory_menu::Menus;
    use mcrs_minecraft_engine_item::ItemCommandsExt;
    use mcrs_minecraft_engine_item::ItemPlugin;
    use mcrs_minecraft_engine_item::Slots;
    use mcrs_minecraft_engine_item::StackCommandsExt;
    use mcrs_minecraft_inventory_file::PlayerItems;
    use mcrs_minecraft_inventory_menu::MenuLayout;
    use mcrs_minecraft_item_stack::Builtin;
    use mcrs_minecraft_item_stack::Count;
    use mcrs_minecraft_item_stack::ItemStack;
    use mcrs_minecraft_item_stack::Kind;
    use mcrs_minecraft_item_stack::MaxStackSize;
    use mcrs_minecraft_item_table::ItemTable;
    use mcrs_minecraft_player_holders::PlayerHolders;
    use mcrs_minecraft_player_holders::PlayerHoldersPlugin;
    use mcrs_minecraft_player_holders::restore_holders;

    use super::*;
    use crate::hotbar::hotbar;
    use crate::item_decorations::WHITE;

    const SIZE: GuiSize = GuiSize {
        width: 427,
        height: 240,
        scale: 2,
    };

    enum Place {
        Cursor,
        CraftResult,
        CraftGrid(u16),
        Player(u16),
    }

    fn player(filled: &[Place]) -> PlayerStacks {
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
        let world = app.world();
        let holders = *world.get::<PlayerHolders>(player).unwrap();
        let menu = world.get::<Menus>(player).unwrap().iter().next().unwrap();
        let layout = world.get::<MenuLayout>(menu).unwrap().clone();
        let parts = *world.get::<MenuParts>(menu).unwrap();
        for place in filled {
            let (holder, index) = match place {
                Place::Cursor => (holders.cursor, 0),
                Place::CraftResult => (holders.craft_result, 0),
                Place::CraftGrid(index) => (holders.craft_grid, *index),
                Place::Player(index) => (player, *index),
            };
            let stack = ItemStack::new(Kind(0), Count::new(1).unwrap());
            let world = app.world_mut();
            world.commands().spawn_stack(stack).put_at(holder, index);
            world.flush();
        }
        PlayerStacks::collect(&layout, &parts, holders.cursor, |slot| {
            app.world()
                .get::<Slots>(slot.holder)?
                .get(usize::from(slot.index))
        })
    }

    #[test]
    fn the_player_stacks_follow_the_vanilla_menu_layout() {
        let placed = [
            (Place::CraftResult, 0),
            (Place::CraftGrid(0), 1),
            (Place::CraftGrid(3), 4),
            (Place::Player(39), 5),
            (Place::Player(36), 8),
            (Place::Player(9), 9),
            (Place::Player(35), 35),
            (Place::Player(0), 36),
            (Place::Player(8), 44),
            (Place::Player(40), 45),
            (Place::Cursor, 46),
        ];
        for (place, menu) in placed {
            let table = player(&[place]);
            let filled: Vec<u16> = (0..=46)
                .filter(|&index| table.get(index).is_some())
                .collect();
            assert_eq!(filled, vec![menu]);
        }
    }

    #[test]
    fn the_hotbar_lists_the_vanilla_rectangles_in_order() {
        let table = player(&[Place::Player(3), Place::Player(40)]);
        let mut out = Vec::new();
        hotbar(SIZE, 3, &table, &mut out);
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
            matches!(out[3], GuiQuad::Item { origin, .. } if origin == IVec2::new(cx - 90 + 62, 221))
        );
        assert!(
            matches!(out[4], GuiQuad::Item { origin, .. } if origin == IVec2::new(cx - 117, 221))
        );
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn the_inventory_screen_lists_the_vanilla_rectangles_in_order() {
        let table = player(&[Place::Player(9), Place::Cursor]);
        let mut out = Vec::new();
        let cursor = IVec2::new(125 + 8 + 5, 37 + 84 + 5);
        inventory_screen(SIZE, cursor, &table, &mut out);
        let top_left = IVec2::new(125, 37);
        assert_eq!(window_corner(SIZE), top_left);
        assert!(
            matches!(out[0], GuiQuad::FillGradient { rect, top: 0xC010_1010, bottom: 0xD010_1010 } if rect == IRect::new(0, 0, 427, 240))
        );
        assert!(
            matches!(out[1], GuiQuad::Sprite { rect, region: "container/inventory", .. } if rect.min == top_left)
        );
        assert_eq!(
            out.get(2),
            Some(&GuiQuad::Figure {
                rect: IRect::new(151, 45, 200, 115)
            }),
            "the figure draws over the image and under the slots and the carried stack"
        );
        assert!(
            matches!(out[3], GuiQuad::Sprite { rect, region: "recipe_book/button", .. } if rect.min == IVec2::new(229, 98))
        );
        assert!(
            matches!(out[4], GuiQuad::Sprite { rect, region: "container/slot_highlight_back", .. } if rect.min == top_left + IVec2::new(4, 80))
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
        let items: Vec<_> = out
            .iter()
            .filter_map(|q| match q {
                GuiQuad::Item { origin, .. } => Some(*origin),
                _ => None,
            })
            .collect();
        assert_eq!(items, vec![top_left + IVec2::new(8, 84), cursor - 8]);
        assert!(matches!(
            out[out.len() - 2],
            GuiQuad::Sprite {
                region: "container/slot_highlight_front",
                ..
            }
        ));
        assert!(matches!(out[out.len() - 1], GuiQuad::Item { .. }));
    }
}
