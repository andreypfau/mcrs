use bevy_ecs::entity::Entity;
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
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_player_slots::Holder;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::clientbound::ClientboundContainerSetContent;
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetHeldSlot;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::VarInt;

use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::full_sends::FullSends;

struct HolderSlots<'slots> {
    inventory: &'slots Slots,
    cursor: Option<&'slots Slots>,
    craft_grid: Option<&'slots Slots>,
    craft_result: Option<&'slots Slots>,
}

impl HolderSlots<'_> {
    fn stack(&self, holder: Holder, index: u16) -> Option<Entity> {
        let slots = match holder {
            Holder::Inventory => Some(self.inventory),
            Holder::Cursor => self.cursor,
            Holder::CraftGrid => self.craft_grid,
            Holder::CraftResult => self.craft_result,
        };
        slots?.get(usize::from(index))
    }
}

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
        let slots = HolderSlots {
            inventory,
            cursor: holders.get(player_holders.cursor).ok(),
            craft_grid: holders.get(player_holders.craft_grid).ok(),
            craft_result: holders.get(player_holders.craft_result).ok(),
        };
        let sync = &mut *sync;
        sync.state_id = sync.state_id.next();
        let mut slot_data = Vec::with_capacity(usize::from(InventoryMenu::LEN));
        for index in 0..InventoryMenu::LEN {
            let stack = InventoryMenu::slot(index)
                .and_then(|slot| slots.stack(slot.holder, slot.index))
                .and_then(|stack| stacks.to_value(stack));
            let sent = SentStack::of(stack, &table, &mut bytes);
            slot_data.push(sent.wire);
            if let Some(view) = sync.view.entry_mut(usize::from(index)) {
                *view = SlotView::Stack(sent.stack);
            }
        }
        let cursor = slots
            .stack(Holder::Cursor, 0)
            .and_then(|stack| stacks.to_value(stack));
        let carried = SentStack::of(cursor, &table, &mut bytes);
        *sync.view.cursor_mut() = SlotView::Stack(carried.stack);
        packets.write(to(
            session,
            PacketPayload::SetHeldSlot(ClientboundSetHeldSlot {
                slot: VarInt(i32::from(selected.0)),
            }),
        ));
        packets.write(to(
            session,
            PacketPayload::ContainerSetContent(ClientboundContainerSetContent {
                container_id: VarInt(i32::from(InventoryMenu::CONTAINER_ID)),
                state_seqno: VarInt(i32::from(sync.state_id.get())),
                slot_data,
                carried_item: carried.wire,
            }),
        ));
    }
}

/// The wire form of a stack and the stack that it gives the client.
struct SentStack {
    wire: ProtoStack<Wire>,
    stack: Option<ItemStack>,
}

impl SentStack {
    fn empty() -> Self {
        Self {
            wire: ProtoStack::default(),
            stack: None,
        }
    }

    /// A stack without a wire form goes as the empty stack.
    // chisle: the two stack models meet through the bytes of the wire; a direct conversion
    // lifts it when the protocol holds `ItemStack`.
    fn of(stack: Option<ItemStack>, table: &ItemTable, bytes: &mut Vec<u8>) -> Self {
        let Some(stack) = stack else {
            return Self::empty();
        };
        bytes.clear();
        if let Err(error) = write_wire(Some(&stack), table, bytes) {
            tracing::warn!("a stack of a player has no wire form: {error:?}");
            return Self::empty();
        }
        match ProtoStack::decode(&mut bytes.as_slice()) {
            Ok(wire) => Self {
                wire,
                stack: Some(stack),
            },
            Err(error) => {
                tracing::warn!("a stack of a player does not decode from its wire form: {error:?}");
                Self::empty()
            }
        }
    }
}
