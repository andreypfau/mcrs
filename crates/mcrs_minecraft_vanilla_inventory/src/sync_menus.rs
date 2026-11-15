use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_inventory_menu::InventoryMenu;
use mcrs_minecraft_inventory_player::Holder;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_wire::VarInt;

use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::holder_slots::HolderSlots;
use crate::menu_content::menu_content;
use crate::menu_queue::MenuQueue;
use crate::sent_stack::SentStack;

#[derive(Default)]
pub(crate) struct SyncScratch {
    hashes: HashScratch,
    bytes: Vec<u8>,
}

/// Compares the inventory menu of each player with its client view in each tick, and sends each
/// entry and the cursor that do not match (INV-SYN-003, INV-SYN-004, ITX-JAV-004). The comparison
/// reads the stacks in place and builds a stack value only to send it. A player in [`MenuQueue`]
/// gets the content of the menu instead (INV-SYN-005).
pub(crate) fn sync_menus(
    mut queue: ResMut<MenuQueue>,
    mut players: Query<(
        Entity,
        &Owner,
        &Slots,
        &PlayerHolders,
        &mut InventoryMenuSync,
    )>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut scratch: Local<SyncScratch>,
) {
    queue.settle();
    let SyncScratch { hashes, bytes } = &mut *scratch;
    for (player, &Owner(session), inventory, player_holders, mut sync) in &mut players {
        let slots = HolderSlots::read(player, inventory, player_holders, &holders);
        if queue.content.binary_search(&player).is_ok() {
            let content = menu_content(&mut sync, &slots, &mut stacks, &table, bytes);
            packets.write(to(session, PacketPayload::ContainerSetContent(content)));
            continue;
        }
        for index in 0..InventoryMenu::LEN {
            let stack =
                InventoryMenu::slot(index).and_then(|slot| slots.stack(slot.holder, slot.index));
            let Some(view) = sync.view.entry(usize::from(index)) else {
                continue;
            };
            let send = match check(view, stack, &mut stacks, &table, hashes) {
                Check::Matches => continue,
                Check::Confirmed(value) => {
                    if let Some(view) = sync.view.entry_mut(usize::from(index)) {
                        *view = SlotView::Stack(value);
                    }
                    continue;
                }
                Check::Differs(value) => value,
            };
            let Ok(slot) = i16::try_from(index) else {
                continue;
            };
            let sent = SentStack::of(send, &table, bytes);
            sync.state_id = sync.state_id.next();
            if let Some(view) = sync.view.entry_mut(usize::from(index)) {
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
        let cursor = slots.stack(Holder::Cursor, 0);
        let send = match check(sync.view.cursor(), cursor, &mut stacks, &table, hashes) {
            Check::Matches => continue,
            Check::Confirmed(value) => {
                *sync.view.cursor_mut() = SlotView::Stack(value);
                continue;
            }
            Check::Differs(value) => value,
        };
        let sent = SentStack::of(send, &table, bytes);
        *sync.view.cursor_mut() = SlotView::Stack(sent.stack);
        let packet = ClientboundSetCursorItem {
            contents: sent.wire,
        };
        packets.write(to(session, PacketPayload::SetCursorItem(packet)));
    }
    queue.content.clear();
}

enum Check {
    Matches,
    /// A hashed view matches this value; the view becomes the value, so that the next
    /// comparison reads the stack in place.
    Confirmed(Option<ItemStack>),
    /// The view does not match; the value is the stack to send.
    Differs(Option<ItemStack>),
}

fn check(
    view: &SlotView,
    stack: Option<Entity>,
    stacks: &mut Stacks,
    table: &ItemTable,
    hashes: &mut HashScratch,
) -> Check {
    let matches = match (view, stack) {
        (SlotView::Unknown, _) => false,
        (SlotView::Stack(None), None) => true,
        (SlotView::Stack(Some(sent)), Some(stack)) => stacks.equals_value(stack, sent),
        (SlotView::Stack(None), Some(stack)) => stacks.kind(stack).is_none(),
        (SlotView::Stack(Some(_)), None) => false,
        (SlotView::Hashed(_), _) => {
            let value = stack.and_then(|stack| stacks.to_value(stack));
            return if view.matches(value.as_ref(), table, hashes) {
                Check::Confirmed(value)
            } else {
                Check::Differs(value)
            };
        }
    };
    if matches {
        Check::Matches
    } else {
        Check::Differs(stack.and_then(|stack| stacks.to_value(stack)))
    }
}
