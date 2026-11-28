use bevy_ecs::message::MessageWriter;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_inventory_menu::MenuOpened;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_protocol_game::clientbound::ClientboundOpenScreen;
use mcrs_minecraft_protocol_game::text::RawText;
use mcrs_minecraft_wire::VarInt;

use crate::MenuSync;
use crate::creative_drops::CreativeDrops;
use crate::cursor_slot::cursor_slot;
use crate::full_sends::FullSends;
use crate::last_container_id::LastContainerId;
use crate::menu_content::menu_content;
use crate::menu_slots::MenuSlots;
use crate::wire_names::WireNames;
use mcrs_minecraft_player_holders::PlayerHolders;

const INVENTORY_CONTAINER_ID: u8 = 0;

pub(crate) fn open_menu_sync(
    opened: On<MenuOpened>,
    viewers: Query<(&Menus, Option<&Owner>, Option<&PlayerHolders>)>,
    mut counters: Query<&mut LastContainerId>,
    menus: Query<(&MenuLayout, &MenuParts, Option<&MenuTitle>)>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    wire_names: WireNames,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut bytes: Local<Vec<u8>>,
    mut sends: ResMut<FullSends>,
    mut commands: Commands,
) {
    let MenuOpened { viewer, menu } = *opened;
    let Ok((viewer_menus, owner, player_holders)) = viewers.get(viewer) else {
        return;
    };
    let Ok((layout, parts, title)) = menus.get(menu) else {
        return;
    };
    let Some(kind) = layout.kind() else {
        if viewer_menus.iter().next() == Some(menu) {
            let sync = MenuSync::new(INVENTORY_CONTAINER_ID, layout.len());
            commands.entity(menu).insert(sync);
            commands
                .entity(viewer)
                .insert_if_new((CreativeDrops::default(), LastContainerId::default()));
            if !sends.0.contains(&viewer) {
                sends.0.push(viewer);
            }
        }
        return;
    };
    let Ok(mut last) = counters.get_mut(viewer) else {
        return;
    };
    let container_id = last.advance();
    let mut sync = MenuSync::new(container_id, layout.len());
    if let Some(&Owner(session)) = owner
        && let Some(title) = title
            .and_then(|MenuTitle(text)| RawText::from_text(text).ok())
            .or_else(|| RawText::from_text(&MenuTitle::default().0).ok())
    {
        let screen = ClientboundOpenScreen {
            container_id: VarInt(i32::from(container_id)),
            menu_type: kind,
            title,
        };
        packets.write(to(session, PacketPayload::OpenScreen(screen)));
        let content = menu_content(
            &mut sync,
            MenuSlots { layout, parts },
            player_holders.map(cursor_slot),
            &holders,
            &mut stacks,
            &wire_names.names(),
            &mut bytes,
        );
        packets.write(to(session, PacketPayload::ContainerSetContent(content)));
    }
    commands.entity(menu).insert(sync);
}
