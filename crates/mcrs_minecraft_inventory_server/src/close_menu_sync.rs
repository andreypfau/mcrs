use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_inventory_menu::MenuClosed;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerClose;
use mcrs_minecraft_wire::VarInt;

use crate::MenuSync;
use crate::close_origin::CloseOrigin;
use crate::menu_slots::MenuSlots;

pub(crate) fn close_menu_sync(
    closed: On<MenuClosed>,
    viewers: Query<(&Menus, Option<&Owner>)>,
    mut menus: Query<(&MenuLayout, &MenuParts, &mut MenuSync)>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
) {
    let MenuClosed { viewer, menu } = *closed;
    let Ok((viewer_menus, owner)) = viewers.get(viewer) else {
        return;
    };
    let Some(first) = viewer_menus.iter().next() else {
        return;
    };
    if first == menu {
        return;
    }
    let Ok([closing, inventory]) = menus.get_many_mut([menu, first]) else {
        return;
    };
    let (closing_layout, closing_parts, closing_sync) = closing;
    let (first_layout, first_parts, mut first_sync) = inventory;
    let closing_slots = MenuSlots {
        layout: closing_layout,
        parts: closing_parts,
    };
    let first_slots = MenuSlots {
        layout: first_layout,
        parts: first_parts,
    };
    for index in 0..first_slots.len() {
        let Some(slot) = first_slots.slot(index) else {
            continue;
        };
        let shared =
            (0..closing_slots.len()).find(|&other| closing_slots.slot(other) == Some(slot));
        let Some(view) = shared.and_then(|other| closing_sync.view.entry(usize::from(other)))
        else {
            continue;
        };
        if let Some(target) = first_sync.view.entry_mut(usize::from(index)) {
            target.clone_from(view);
        }
    }
    *first_sync.view.cursor_mut() = SlotView::Stack(None);
    if closing_sync.close_origin == CloseOrigin::Server
        && let Some(&Owner(session)) = owner
    {
        let packet = ClientboundContainerClose {
            container_id: VarInt(i32::from(closing_sync.container_id)),
        };
        packets.write(to(session, PacketPayload::ContainerClose(packet)));
    }
}
