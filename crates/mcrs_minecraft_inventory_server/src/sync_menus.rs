use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_engine_inventory_menu::MenuOf;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetSlot;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetCursorItem;
use mcrs_minecraft_wire::VarInt;

use crate::MenuSync;
use crate::cursor_slot::cursor_slot;
use crate::menu_content::menu_content;
use crate::menu_queue::MenuQueue;
use crate::menu_slots::MenuSlots;
use crate::sent_stack::SentStack;
use crate::slot_stack::slot_stack;
use crate::wire_names::WireNames;
use mcrs_minecraft_player_holders::PlayerHolders;

#[derive(Default)]
pub(crate) struct SyncScratch {
    hashes: HashScratch,
    bytes: Vec<u8>,
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the slot list of a full content send moves into the outbound packet"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(1))]
pub(crate) fn sync_menus(
    mut queue: ResMut<MenuQueue>,
    mut menus: Query<(Entity, &MenuOf, &MenuLayout, &MenuParts, &mut MenuSync)>,
    viewers: Query<(&Owner, &Menus, Option<&PlayerHolders>)>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    wire_names: WireNames,
    mut packets: MessageWriter<OutboundPlayerPacket>,
    mut scratch: Local<SyncScratch>,
) {
    queue.settle();
    let SyncScratch { hashes, bytes } = &mut *scratch;
    let names = wire_names.names();
    for (menu, &MenuOf(viewer), layout, parts, mut sync) in &mut menus {
        let Ok((&Owner(session), viewer_menus, player_holders)) = viewers.get(viewer) else {
            continue;
        };
        if viewer_menus.open() != Some(menu) {
            continue;
        }
        let menu_slots = MenuSlots { layout, parts };
        let cursor = player_holders.map(cursor_slot);
        if queue.content.binary_search(&menu).is_ok() {
            let content = menu_content(
                &mut sync,
                menu_slots,
                cursor,
                &holders,
                &mut stacks,
                &names,
                bytes,
            );
            packets.write(to(session, PacketPayload::ContainerSetContent(content)));
            continue;
        }
        for index in 0..menu_slots.len() {
            let stack = menu_slots
                .slot(index)
                .and_then(|slot| slot_stack(&holders, slot));
            let Some(view) = sync.view.entry(usize::from(index)) else {
                continue;
            };
            let send = match check(view, stack, &mut stacks, &names, hashes) {
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
            let sent = SentStack::of(send, &names, bytes);
            sync.state_id = sync.state_id.next();
            if let Some(view) = sync.view.entry_mut(usize::from(index)) {
                *view = SlotView::Stack(sent.stack);
            }
            let packet = ClientboundContainerSetSlot {
                container_id: VarInt(i32::from(sync.container_id)),
                state_seqno: VarInt(i32::from(sync.state_id.get())),
                slot,
                item: sent.wire,
            };
            packets.write(to(session, PacketPayload::ContainerSetSlot(packet)));
        }
        let cursor = cursor.and_then(|slot| slot_stack(&holders, slot));
        let send = match check(sync.view.cursor(), cursor, &mut stacks, &names, hashes) {
            Check::Matches => continue,
            Check::Confirmed(value) => {
                *sync.view.cursor_mut() = SlotView::Stack(value);
                continue;
            }
            Check::Differs(value) => value,
        };
        let sent = SentStack::of(send, &names, bytes);
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
    Confirmed(Option<ItemStack>),
    Differs(Option<ItemStack>),
}

fn check(
    view: &SlotView,
    stack: Option<Entity>,
    stacks: &mut Stacks,
    names: &RegistryNames<'_>,
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
            return if view.matches(value.as_ref(), names, hashes) {
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
