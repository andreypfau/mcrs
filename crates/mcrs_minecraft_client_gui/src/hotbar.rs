use bevy::math::IVec2;
use bevy::prelude::*;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_mode::PlayerGameMode;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item::slots;
use mcrs_minecraft_item_icon::Overlays;
use mcrs_minecraft_player_holders::PlayerHolders;

use crate::draw_gate::{changed_or_removed, layer_stale};
use crate::gui_band::GuiBand;
use crate::gui_layer::{GuiLayer, spawn_layer};
use crate::gui_phase::GuiPhase;
use crate::gui_quad::sprite;
use crate::gui_size::GuiSize;
use crate::item_icons::ItemIcons;
use crate::menu_stacks::MenuStacks;
use crate::slot_icon::SlotIcon;

pub fn hotbar(
    layer: &mut GuiLayer,
    size: GuiSize,
    selected: u8,
    stacks: &MenuStacks,
    icon_of: impl Fn(Entity) -> Option<SlotIcon>,
) {
    let (cx, h) = (size.width / 2, size.height);
    let offhand = stacks.get(slots::OFFHAND);
    layer.push(sprite(
        IVec2::new(cx - 91, h - 22),
        IVec2::new(182, 22),
        "hud/hotbar",
    ));
    layer.push(sprite(
        IVec2::new(cx - 91 - 1 + i32::from(selected) * 20, h - 22 - 1),
        IVec2::new(24, 23),
        "hud/hotbar_selection",
    ));
    if offhand.is_some() {
        layer.push(sprite(
            IVec2::new(cx - 91 - 29, h - 23),
            IVec2::new(29, 24),
            "hud/hotbar_offhand_left",
        ));
    }
    let y = h - 16 - 3;
    let mut draw = |stack: Option<Entity>, origin: IVec2| {
        if let Some(slot) = stack.and_then(&icon_of) {
            ItemIcons::draw_icon(layer, origin, slot.icon, slot.overlay, Overlays::Stack);
        }
    };
    for i in 0..9u16 {
        let origin = IVec2::new(cx - 90 + i32::from(i) * 20 + 2, y);
        draw(stacks.get(slots::HOTBAR.start + i), origin);
    }
    draw(offhand, IVec2::new(cx - 91 - 26, y));
}

#[derive(Component, Default)]
#[require(GuiLayer = GuiLayer::new(GuiBand::Hud, 1))]
pub struct HotbarLayer;

#[derive(Default)]
pub(crate) struct HotbarStacks(MenuStacks);

pub(crate) fn hotbar_draw(
    player: Query<(&PlayerHolders, &Menus, &Selected, Option<&PlayerGameMode>)>,
    menus: Query<(&MenuLayout, &MenuParts)>,
    holders: Query<&Slots>,
    icons: Query<&SlotIcon>,
    mut stacks: Local<HotbarStacks>,
    size: Res<GuiSize>,
    mut layer: Single<&mut GuiLayer, With<HotbarLayer>>,
) {
    layer.redraw();
    let Ok((player, player_menus, selected, mode)) = player.single() else {
        return;
    };
    let occupant = |slot: SlotRef| holders.get(slot.holder).ok()?.get(usize::from(slot.index));
    let icon_of = |stack| icons.get(stack).ok().copied();
    if mode.is_some_and(|PlayerGameMode(mode)| *mode == GameMode::Spectator) {
        return;
    }
    let Some((layout, parts)) = player_menus
        .iter()
        .next()
        .and_then(|menu| menus.get(menu).ok())
    else {
        return;
    };
    stacks.0.collect(layout, parts, player.cursor, occupant);
    hotbar(&mut layer, *size, selected.0, &stacks.0, icon_of);
}

pub struct HotbarPlugin;

impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_layer::<HotbarLayer>)
            .add_systems(
                PostUpdate,
                hotbar_draw
                    .run_if(
                        layer_stale::<HotbarLayer>()
                            .or_else(changed_or_removed::<PlayerHolders>())
                            .or_else(changed_or_removed::<Menus>())
                            .or_else(changed_or_removed::<Selected>())
                            .or_else(changed_or_removed::<PlayerGameMode>())
                            .or_else(changed_or_removed::<MenuLayout>())
                            .or_else(changed_or_removed::<MenuParts>())
                            .or_else(changed_or_removed::<Slots>())
                            .or_else(changed_or_removed::<SlotIcon>()),
                    )
                    .in_set(GuiPhase::Draw),
            );
    }
}
