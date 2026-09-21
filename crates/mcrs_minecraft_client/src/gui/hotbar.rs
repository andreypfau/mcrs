use bevy::math::{IRect, IVec2};
use mcrs_minecraft_item::{SlotTable, slots};

use super::item_decorations::WHITE;
use super::scene::{GuiQuad, GuiSize};

fn sprite(x: i32, y: i32, w: i32, h: i32, region: &'static str) -> GuiQuad {
    GuiQuad::Sprite {
        rect: IRect::new(x, y, x + w, y + h),
        region,
        color: WHITE,
    }
}

/// The main arm is right (no options yet), so the offhand frame sits on the left.
pub fn hotbar(size: GuiSize, selected: u8, slots: &SlotTable, out: &mut Vec<GuiQuad>) {
    let (cx, h) = (size.width / 2, size.height);
    let offhand = slots.get(slots::OFFHAND);
    out.push(sprite(cx - 91, h - 22, 182, 22, "hud/hotbar"));
    out.push(sprite(
        cx - 91 - 1 + i32::from(selected) * 20,
        h - 22 - 1,
        24,
        23,
        "hud/hotbar_selection",
    ));
    if offhand.is_some() {
        out.push(sprite(
            cx - 91 - 29,
            h - 23,
            29,
            24,
            "hud/hotbar_offhand_left",
        ));
    }
    let y = h - 16 - 3;
    for i in 0..9u16 {
        if let Some(stack) = slots.get(slots::HOTBAR.start + i) {
            out.push(GuiQuad::Item {
                origin: IVec2::new(cx - 90 + i32::from(i) * 20 + 2, y),
                stack,
            });
        }
    }
    if let Some(stack) = offhand {
        out.push(GuiQuad::Item {
            origin: IVec2::new(cx - 91 - 26, y),
            stack,
        });
    }
}
