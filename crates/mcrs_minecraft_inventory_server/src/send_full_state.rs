use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetHeldSlot;
use mcrs_minecraft_wire::VarInt;

use crate::MenuSync;
use crate::cursor_slot::cursor_slot;
use crate::full_sends::FullSends;
use crate::menu_content::menu_content;
use crate::menu_slots::MenuSlots;
use crate::sent_selected::SentSelected;
use crate::wire_names::WireNames;
use mcrs_minecraft_player_holders::PlayerHolders;

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the slot list of a full content send moves into the outbound packet"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
pub(crate) fn send_full_state(
    mut sends: ResMut<FullSends>,
    players: Query<(&Owner, &Selected, &Menus, Option<&PlayerHolders>)>,
    holders: Query<&Slots>,
    mut menus: Query<(&MenuLayout, &MenuParts, &mut MenuSync)>,
    mut stacks: Stacks,
    wire_names: WireNames,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut bytes: Local<Vec<u8>>,
    mut commands: Commands,
) {
    let names = wire_names.names();
    for player in sends.0.drain(..) {
        let Ok((&Owner(session), selected, player_menus, player_holders)) = players.get(player)
        else {
            continue;
        };
        let Some(first) = player_menus.iter().next() else {
            continue;
        };
        let Ok((layout, parts, mut sync)) = menus.get_mut(first) else {
            continue;
        };
        let content = menu_content(
            &mut sync,
            MenuSlots { layout, parts },
            player_holders.map(cursor_slot),
            &holders,
            &mut stacks,
            &names,
            &mut bytes,
        );
        packets.write(to(
            session,
            PacketPayload::SetHeldSlot(ClientboundSetHeldSlot {
                slot: VarInt(i32::from(selected.0)),
            }),
        ));
        packets.write(to(session, PacketPayload::ContainerSetContent(content)));
        commands.entity(player).insert(SentSelected(selected.0));
    }
}
