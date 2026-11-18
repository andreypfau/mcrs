use bevy::math::IVec2;
use bevy::prelude::Entity;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_item_icon::Overlays;

use crate::item_icons::ItemIcons;
use crate::menu_stacks::MenuStacks;
use crate::scene::{GuiScene, sprite};
use crate::slot_icon::SlotIcon;

pub fn hotbar(
    scene: &mut GuiScene,
    selected: u8,
    stacks: &MenuStacks,
    icon_of: impl Fn(Entity) -> Option<SlotIcon>,
) {
    let (cx, h) = (scene.size.width / 2, scene.size.height);
    let offhand = stacks.get(slots::OFFHAND);
    scene.quads.push(sprite(
        IVec2::new(cx - 91, h - 22),
        IVec2::new(182, 22),
        "hud/hotbar",
    ));
    scene.quads.push(sprite(
        IVec2::new(cx - 91 - 1 + i32::from(selected) * 20, h - 22 - 1),
        IVec2::new(24, 23),
        "hud/hotbar_selection",
    ));
    if offhand.is_some() {
        scene.quads.push(sprite(
            IVec2::new(cx - 91 - 29, h - 23),
            IVec2::new(29, 24),
            "hud/hotbar_offhand_left",
        ));
    }
    let y = h - 16 - 3;
    let mut draw = |stack: Option<Entity>, origin: IVec2| {
        if let Some(slot) = stack.and_then(&icon_of) {
            ItemIcons::draw_icon(scene, origin, slot.icon, slot.overlay, Overlays::Stack);
        }
    };
    for i in 0..9u16 {
        let origin = IVec2::new(cx - 90 + i32::from(i) * 20 + 2, y);
        draw(stacks.get(slots::HOTBAR.start + i), origin);
    }
    draw(offhand, IVec2::new(cx - 91 - 26, y));
}
