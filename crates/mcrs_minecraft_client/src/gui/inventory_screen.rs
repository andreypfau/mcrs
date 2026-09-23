use bevy::math::{IRect, IVec2};
use mcrs_minecraft_item::{SlotTable, slots};

use super::scene::{GuiQuad, GuiSize, sprite};

pub const IMAGE_WIDTH: i32 = 176;
pub const IMAGE_HEIGHT: i32 = 166;
const DIM_TOP: u32 = 0xC010_1010;
const DIM_BOTTOM: u32 = 0xD010_1010;

pub fn slot_position(menu: u16) -> IVec2 {
    let i = i32::from(menu);
    match menu {
        slots::RESULT => IVec2::new(154, 28),
        m if slots::CRAFT.contains(&m) => IVec2::new(98 + (i - 1) % 2 * 18, 18 + (i - 1) / 2 * 18),
        slots::ARMOR_HEAD..=slots::ARMOR_FEET => IVec2::new(8, 8 + (i - 5) * 18),
        m if slots::MAIN.contains(&m) => IVec2::new(8 + (i - 9) % 9 * 18, 84 + (i - 9) / 9 * 18),
        m if slots::HOTBAR.contains(&m) => IVec2::new(8 + (i - 36) * 18, 142),
        slots::OFFHAND => IVec2::new(77, 62),
        _ => unreachable!("{menu} is not a menu slot"),
    }
}

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

pub fn origin(size: GuiSize) -> IVec2 {
    IVec2::new(
        (size.width - IMAGE_WIDTH) / 2,
        (size.height - IMAGE_HEIGHT) / 2,
    )
}

fn hovering(slot: IVec2, local: IVec2) -> bool {
    local.x >= slot.x - 1 && local.x < slot.x + 17 && local.y >= slot.y - 1 && local.y < slot.y + 17
}

pub fn hovered_slot(size: GuiSize, cursor: IVec2) -> Option<u16> {
    let local = cursor - origin(size);
    (0..slots::MENU_COUNT as u16).find(|&menu| hovering(slot_position(menu), local))
}

pub fn inventory_screen(size: GuiSize, cursor: IVec2, slots: &SlotTable, out: &mut Vec<GuiQuad>) {
    let top_left = origin(size);
    out.push(GuiQuad::FillGradient {
        rect: IRect::new(0, 0, size.width, size.height),
        top: DIM_TOP,
        bottom: DIM_BOTTOM,
    });
    out.push(sprite(
        top_left,
        IVec2::new(IMAGE_WIDTH, IMAGE_HEIGHT),
        "container/inventory",
    ));
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
    let hovered = hovered_slot(size, cursor);
    if let Some(menu) = hovered {
        out.push(sprite(
            top_left + slot_position(menu) - 4,
            IVec2::new(24, 24),
            "container/slot_highlight_back",
        ));
    }
    for menu in 0..slots::MENU_COUNT as u16 {
        let at = top_left + slot_position(menu);
        match slots.get(menu) {
            Some(stack) => out.push(GuiQuad::Item { origin: at, stack }),
            None => {
                if let Some(icon) = empty_icon(menu) {
                    out.push(sprite(at, IVec2::splat(16), icon));
                }
            }
        }
    }
    if let Some(menu) = hovered {
        out.push(sprite(
            top_left + slot_position(menu) - 4,
            IVec2::new(24, 24),
            "container/slot_highlight_front",
        ));
    }
    if let Some(stack) = slots.get(slots::CARRIED) {
        out.push(GuiQuad::Item {
            origin: cursor - 8,
            stack,
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;
    use mcrs_minecraft_item::Held;

    use super::super::hotbar::hotbar;
    use super::super::item_decorations::WHITE;
    use super::*;

    const SIZE: GuiSize = GuiSize {
        width: 427,
        height: 240,
        scale: 2,
    };

    #[test]
    fn slot_positions_match_the_vanilla_menu() {
        assert_eq!(slot_position(0), IVec2::new(154, 28));
        assert_eq!(slot_position(1), IVec2::new(98, 18));
        assert_eq!(slot_position(4), IVec2::new(116, 36));
        assert_eq!(slot_position(5), IVec2::new(8, 8));
        assert_eq!(slot_position(8), IVec2::new(8, 62));
        assert_eq!(slot_position(9), IVec2::new(8, 84));
        assert_eq!(slot_position(35), IVec2::new(152, 120));
        assert_eq!(slot_position(36), IVec2::new(8, 142));
        assert_eq!(slot_position(44), IVec2::new(152, 142));
        assert_eq!(slot_position(45), IVec2::new(77, 62));
    }

    fn player(world: &mut World, filled: &[u16]) -> SlotTable {
        let holder = world.spawn(SlotTable::fixed(slots::COUNT)).id();
        for &index in filled {
            world.spawn(Held { holder, index });
        }
        world.get::<SlotTable>(holder).unwrap().clone()
    }

    #[test]
    fn the_hotbar_lists_the_vanilla_rectangles_in_order() {
        let mut world = World::new();
        let table = player(&mut world, &[slots::HOTBAR.start + 3, slots::OFFHAND]);
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
        let mut world = World::new();
        let table = player(&mut world, &[slots::MAIN.start, slots::CARRIED]);
        let mut out = Vec::new();
        let cursor = IVec2::new(125 + 8 + 5, 37 + 84 + 5);
        inventory_screen(SIZE, cursor, &table, &mut out);
        let top_left = IVec2::new(125, 37);
        assert_eq!(origin(SIZE), top_left);
        assert!(
            matches!(out[0], GuiQuad::FillGradient { rect, top: 0xC010_1010, bottom: 0xD010_1010 } if rect == IRect::new(0, 0, 427, 240))
        );
        assert!(
            matches!(out[1], GuiQuad::Sprite { rect, region: "container/inventory", .. } if rect.min == top_left)
        );
        assert!(
            matches!(out[2], GuiQuad::Sprite { rect, region: "recipe_book/button", .. } if rect.min == IVec2::new(229, 98))
        );
        assert!(
            matches!(out[3], GuiQuad::Sprite { rect, region: "container/slot_highlight_back", .. } if rect.min == top_left + IVec2::new(4, 80))
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

    #[test]
    fn hover_uses_the_one_pixel_margin_and_the_first_slot_wins() {
        assert_eq!(
            hovered_slot(SIZE, origin(SIZE) + IVec2::new(7, 7)),
            Some(slots::ARMOR_HEAD)
        );
        assert_eq!(
            hovered_slot(SIZE, origin(SIZE) + IVec2::new(8, 24)),
            Some(slots::ARMOR_HEAD)
        );
        assert_eq!(
            hovered_slot(SIZE, origin(SIZE) + IVec2::new(8, 25)),
            Some(slots::ARMOR_CHEST)
        );
        assert_eq!(hovered_slot(SIZE, IVec2::new(-1, -1)), None);
    }
}
