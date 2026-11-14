use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::message::Messages;
use bevy_ecs::observer::On;
use bevy_ecs::system::Commands;
use bevy_ecs::system::InRef;
use bevy_ecs::system::Local;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::ability::PlayerGameMode;
use mcrs_minecraft_engine::game_profile::GameProfile;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_entity::Cause;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_menu::InventoryMenu;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;
use mcrs_minecraft_wire::Encode;

use crate::InventoryInput;
use crate::InventoryMenuSync;
use crate::PlayerHolders;
use crate::creative_drops::CreativeDrops;
use crate::insert_stack::max_stack_size;
use crate::launch_drops::launch_from_player;
use crate::menu_queue::MenuQueue;
use crate::request_ids::CREATIVE_REQUEST;

/// Writes the [`InventoryInput::CreativeSet`] of each `set_creative_mode_slot` packet whose stack
/// decodes.
pub(crate) fn set_creative_mode_slot(
    received: On<Received<ServerboundSetCreativeModeSlot>>,
    table: Res<ItemTable>,
    mut inputs: MessageWriter<InventoryInput>,
    mut bytes: Local<Vec<u8>>,
) {
    let Some(packet) = received.packet() else {
        return;
    };
    bytes.clear();
    if packet.item.encode(&mut *bytes).is_err() {
        return;
    }
    // STK-NET-009: a stack without a saved form is a decode error of the packet.
    let Ok(stack) = read_wire_with_length(&mut bytes.as_slice(), &*table) else {
        return;
    };
    inputs.write(InventoryInput::CreativeSet {
        player: received.entity,
        slot: packet.slot,
        stack,
    });
}

/// Applies a creative set that passes the checks of INV-CRE-003: a set of an entry of the
/// inventory menu (INV-CRE-004), or a creative drop (IEN-SPN-007). A set that fails a check is a
/// claim of the client for its entry (INV-CRE-006).
#[expect(
    clippy::type_complexity,
    reason = "one query reads each part of the player that the packet needs"
)]
pub(crate) fn apply_creative_set(
    InRef(input): InRef<InventoryInput>,
    mut players: Query<(
        &PlayerHolders,
        &mut InventoryMenuSync,
        Option<&PlayerGameMode>,
        &mut CreativeDrops,
        Option<&Transform>,
        Option<&GameProfile>,
    )>,
    table: Res<ItemTable>,
    clock: Res<BirthClock>,
    mut queue: ResMut<MenuQueue>,
    mut commands: Commands,
) {
    let &InventoryInput::CreativeSet {
        player,
        slot,
        ref stack,
    } = input
    else {
        return;
    };
    let Ok((holders, mut sync, game_mode, mut drops, transform, profile)) = players.get_mut(player)
    else {
        return;
    };
    let stack = stack.as_ref();
    let creative = game_mode.is_some_and(|game_mode| game_mode.0 == GameMode::Creative);
    let fits = stack.is_none_or(|stack| stack.count.get() <= max_stack_size(stack, &table));
    let entry = u16::try_from(slot)
        .ok()
        .filter(|index| (1..InventoryMenu::LEN).contains(index))
        .and_then(InventoryMenu::slot);
    if creative && fits && slot < 0 {
        if drops.try_drop(clock.0)
            && let Some(stack) = stack
        {
            let mut entity = commands.spawn_stack(stack.clone());
            launch_from_player(&mut entity, transform, profile, Cause::Throw);
        }
        return;
    }
    if creative
        && fits
        && let Some(entry) = entry
    {
        let target = SlotRef {
            holder: holders.entity(player, entry.holder),
            index: entry.index,
        };
        let spawned = stack.map(|stack| commands.spawn_stack(stack.clone()).id());
        let count = stack.map_or(0, |stack| stack.count.get());
        let request = Request::new(CREATIVE_REQUEST).set(target, spawned, count);
        commands.entity(player).request(request);
        if let Some(spawned) = spawned {
            commands.queue(move |world: &mut World| despawn_if_rejected(world, player, spawned));
        }
        set_view(&mut sync, slot, stack);
        return;
    }
    if set_view(&mut sync, slot, stack) {
        queue.compare.push(player);
    }
}

/// Despawns `stack` when the last request result is a rejection of the creative set of `player`,
/// since a rejected set gives the stack no slot.
fn despawn_if_rejected(world: &mut World, player: Entity, stack: Entity) {
    let rejected = (world.get_resource::<Messages<RequestResult>>())
        .and_then(|results| results.iter_current_update_messages().last())
        .is_some_and(|result| {
            result.holder == player && result.id == CREATIVE_REQUEST && result.outcome.is_err()
        });
    if rejected {
        world.despawn(stack);
    }
}

/// Makes `stack` the client view of the entry `slot`. Gives false when the inventory menu has no
/// such entry.
fn set_view(sync: &mut InventoryMenuSync, slot: i16, stack: Option<&ItemStack>) -> bool {
    let view = usize::try_from(slot)
        .ok()
        .and_then(|index| sync.view.entry_mut(index));
    let Some(view) = view else {
        return false;
    };
    *view = SlotView::Stack(stack.cloned());
    true
}
