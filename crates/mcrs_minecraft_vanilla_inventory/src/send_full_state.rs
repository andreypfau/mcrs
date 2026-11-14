use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetHeldSlot;
use mcrs_minecraft_wire::VarInt;

use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::full_sends::FullSends;
use crate::holder_slots::HolderSlots;
use crate::menu_content::menu_content;

/// Sends each restored player its selected slot and the content of its inventory menu with the
/// cursor, and records them as its client view (INV-SEL-003, INV-SYN-005).
#[expect(
    clippy::too_many_arguments,
    reason = "the send reads the holders, the stacks and the wire names, and writes the view"
)]
pub(crate) fn send_full_state(
    mut sends: ResMut<FullSends>,
    players: Query<(&Owner, &Slots, &Selected, &PlayerHolders)>,
    holders: Query<&Slots>,
    mut syncs: Query<&mut InventoryMenuSync>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut bytes: Local<Vec<u8>>,
) {
    for player in sends.0.drain(..) {
        let Ok((&Owner(session), inventory, selected, player_holders)) = players.get(player) else {
            continue;
        };
        let Ok(mut sync) = syncs.get_mut(player) else {
            continue;
        };
        let slots = HolderSlots::read(player, inventory, player_holders, &holders);
        let content = menu_content(&mut sync, &slots, &mut stacks, &table, &mut bytes);
        packets.write(to(
            session,
            PacketPayload::SetHeldSlot(ClientboundSetHeldSlot {
                slot: VarInt(i32::from(selected.0)),
            }),
        ));
        packets.write(to(session, PacketPayload::ContainerSetContent(content)));
    }
}
