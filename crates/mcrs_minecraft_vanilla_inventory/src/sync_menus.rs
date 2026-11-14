use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_item::Given;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_inventory_view::Agreement;
use mcrs_minecraft_inventory_view::Mismatches;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_vanilla_item_entity::PickedUp;
use mcrs_minecraft_wire::VarInt;

use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::holder_slots::HolderSlots;
use crate::menu_content::menu_content;
use crate::menu_queue::MenuQueue;
use crate::sent_stack::SentStack;

#[derive(Default)]
pub(crate) struct SyncScratch {
    entries: Vec<Option<ItemStack>>,
    hashes: HashScratch,
    mismatches: Mismatches,
    bytes: Vec<u8>,
}

/// Sends the inventory menu of each queued player: in full, or each entry and the cursor that
/// do not match the client view (INV-SYN-003, INV-SYN-004, ITX-JAV-004). A request result, a
/// give or a pickup on a holder of a player queues the player for a comparison.
#[expect(
    clippy::too_many_arguments,
    reason = "the sync reads the touched holders and their stacks, and writes the view and packets"
)]
pub(crate) fn sync_menus(
    mut results: MessageReader<RequestResult>,
    mut given: MessageReader<Given>,
    mut picked: MessageReader<PickedUp>,
    mut queue: ResMut<MenuQueue>,
    parents: Query<&ChildOf>,
    players: Query<(&Owner, &Slots, &PlayerHolders)>,
    holders: Query<&Slots>,
    mut syncs: Query<&mut InventoryMenuSync>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut scratch: Local<SyncScratch>,
) {
    let queue = &mut *queue;
    let touched = (results.read().map(|result| result.holder))
        .chain(given.read().map(|given| given.holder))
        .chain(picked.read().map(|picked| picked.collector));
    for holder in touched {
        let player = if players.contains(holder) {
            Some(holder)
        } else {
            (parents.get(holder).ok().map(ChildOf::parent))
                .filter(|parent| players.contains(*parent))
        };
        if let Some(player) = player {
            queue.compare.push(player);
        }
    }
    queue.settle();
    let SyncScratch {
        entries,
        hashes,
        mismatches,
        bytes,
    } = &mut *scratch;
    let queued = (queue
        .content
        .drain(..)
        .map(|player| (player, MenuSend::Content)))
    .chain(
        queue
            .compare
            .drain(..)
            .map(|player| (player, MenuSend::Mismatches)),
    );
    for (player, send) in queued {
        let (Ok((&Owner(session), inventory, player_holders)), Ok(mut sync)) =
            (players.get(player), syncs.get_mut(player))
        else {
            continue;
        };
        let sync = &mut *sync;
        let slots = HolderSlots::read(player, inventory, player_holders, &holders);
        if send == MenuSend::Content {
            let content = menu_content(sync, &slots, &mut stacks, &table, bytes);
            packets.write(to(session, PacketPayload::ContainerSetContent(content)));
            continue;
        }
        // chisle: each compared player costs 46 stack values built from their rows, mismatch or
        // not; a view comparison that reads the stack rows in place lifts it.
        entries.clear();
        for index in 0..InventoryMenu::LEN {
            let stack = InventoryMenu::slot(index)
                .and_then(|slot| slots.stack(slot.holder, slot.index))
                .and_then(|stack| stacks.to_value(stack));
            entries.push(stack);
        }
        let cursor = slots
            .stack(Holder::Cursor, 0)
            .and_then(|stack| stacks.to_value(stack));
        sync.view.compare(
            entries.iter().map(Option::as_ref),
            cursor.as_ref(),
            &*table,
            hashes,
            mismatches,
        );
        for &index in mismatches.entries() {
            let (Ok(slot), Some(stack)) = (i16::try_from(index), entries.get_mut(index)) else {
                continue;
            };
            let sent = SentStack::of(stack.take(), &table, bytes);
            sync.state_id = sync.state_id.next();
            if let Some(view) = sync.view.entry_mut(index) {
                *view = SlotView::Stack(sent.stack);
            }
            let packet = ClientboundContainerSetSlot {
                container_id: VarInt(i32::from(InventoryMenu::CONTAINER_ID)),
                state_seqno: VarInt(i32::from(sync.state_id.get())),
                slot,
                item: sent.wire,
            };
            packets.write(to(session, PacketPayload::ContainerSetSlot(packet)));
        }
        if mismatches.cursor() == Agreement::Differs {
            let sent = SentStack::of(cursor, &table, bytes);
            *sync.view.cursor_mut() = SlotView::Stack(sent.stack);
            let packet = ClientboundSetCursorItem {
                contents: sent.wire,
            };
            packets.write(to(session, PacketPayload::SetCursorItem(packet)));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuSend {
    Content,
    Mismatches,
}
